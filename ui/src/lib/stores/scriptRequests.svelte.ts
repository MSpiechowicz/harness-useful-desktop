import { t } from '$lib/i18n/i18n.svelte';
import type { ScriptRequestEvent } from '$lib/ipc/bindings/ScriptRequestEvent';
import type { ScriptRunRequest } from '$lib/ipc/bindings/ScriptRunRequest';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { type ToastsStore, toasts } from './toasts.svelte';

/** How long the Memory server waits for an answer before it tells the agent nobody replied. */
export const APPROVAL_WINDOW_MS = 45_000;

/** How much later than the window's end a request that was never reported gone is dropped, so a missed event cannot leave a prompt on screen. */
const EXPIRY_GRACE_MS = 1500;

/** A request that was reported long after the server wrote it is shown for this long from its arrival: the server's clock and ours may disagree, and its window has not ended while the request is still being reported. */
const LATE_WINDOW_MS = 40_000;

/** How many answered or ended request ids are remembered, so a request the backend reports again after a reload is not asked twice. */
const REMEMBERED_ANSWERS = 50;

const ANSWERED_KEY = 'harness.answeredScriptRequests';

interface Waiting {
  request: ScriptRunRequest;
  /** When the server stops waiting, in epoch milliseconds. */
  deadline: number;
}

/**
 * When the server stops waiting for `request`: the window counted from the earlier of the server's `createdAt` and the
 * moment the backend received it (a date ahead of ours counts as now). A request whose `createdAt` makes it look expired
 * but that the backend received a moment ago is still best-effort shown, for less than the window.
 */
function deadlineOf(request: ScriptRunRequest, now: number): number {
  const created = Date.parse(request.createdAt);
  const received = Date.parse(request.receivedAt);
  const known = !Number.isNaN(received);
  const arrival = known ? Math.min(received, now) : now;
  const start = Number.isNaN(created) ? arrival : Math.min(created, arrival);
  const deadline = start + APPROVAL_WINDOW_MS;

  if (deadline <= now && known && now - arrival < APPROVAL_WINDOW_MS) {
    return arrival + LATE_WINDOW_MS;
  }

  return deadline;
}

function loadAnswered(): string[] {
  try {
    const stored: unknown = JSON.parse(window.sessionStorage.getItem(ANSWERED_KEY) ?? '[]');

    return Array.isArray(stored) ? stored.filter((id): id is string => typeof id === 'string').slice(-REMEMBERED_ANSWERS) : [];
  } catch {
    return [];
  }
}

function saveAnswered(ids: string[]): void {
  try {
    window.sessionStorage.setItem(ANSWERED_KEY, JSON.stringify(ids));
  } catch {
    // Storage can be unavailable (a locked-down webview); the memory then lasts until the page reloads.
  }
}

/**
 * Agents asking to run a script this computer has not approved. The Memory server writes a request, the backend reports
 * it, and the answer goes back the same way. Requests wait in a queue and the first one is shown. The queue holds what
 * the server still waits for: a request leaves when it is answered, when the server gives up, or when its window has run out.
 */
export class ScriptRequestsStore {
  private queue = $state.raw<Waiting[]>([]);
  private readonly timers = new Map<string, ReturnType<typeof setTimeout>>();
  /** The last requests answered or ended, oldest first. */
  private answered: string[] = loadAnswered();

  /** True while the answer to the current request is on its way to the backend. */
  answering = $state(false);

  constructor(
    private readonly transport: Transport,
    private readonly notices: ToastsStore,
  ) {}

  /** The request on screen: the one that has waited longest. */
  get current(): ScriptRunRequest | null {
    return this.queue[0]?.request ?? null;
  }

  /** When the server stops waiting for the current request, in epoch milliseconds. */
  get deadline(): number {
    return this.queue[0]?.deadline ?? 0;
  }

  /** How many requests wait behind the current one. */
  get behind(): number {
    return Math.max(0, this.queue.length - 1);
  }

  push(request: ScriptRunRequest, now: number = Date.now()): void {
    const deadline = deadlineOf(request, now);

    // The same request can be reported twice (also after the page reloaded), and one that ran out of time before it arrived has no one waiting.
    if (deadline <= now || this.answered.includes(request.requestId) || this.queue.some((waiting) => waiting.request.requestId === request.requestId)) {
      return;
    }

    this.queue = [...this.queue, { request, deadline }];
    this.timers.set(
      request.requestId,
      setTimeout(() => this.ended(request.requestId), deadline - now + EXPIRY_GRACE_MS),
    );
  }

  /** The server no longer waits for this request (the agent gave up, or its time ran out). It leaves the queue with a quiet note. */
  ended(requestId: string): void {
    const request = this.queue.find((waiting) => waiting.request.requestId === requestId)?.request;
    if (!request) {
      return;
    }

    this.remove(requestId);
    this.notices.push({ title: t('memory.request.gone.title', { name: request.script.name }), description: t('memory.request.gone.body'), tone: 'neutral' });
  }

  /** Sends the user's answer for the current request and moves on to the next one. */
  async decide(allow: boolean): Promise<void> {
    const request = this.current;
    if (!request || this.answering) {
      return;
    }

    // The prompt moves on at once. The server's own end of the wait (it reports the request gone) must not read as a failure.
    this.answering = true;
    this.remove(request.requestId);

    try {
      await this.transport.invoke('memory_script_decide', { requestId: request.requestId, allow });
    } catch (error) {
      const failure = IpcFailure.from(error);

      // The agent gave up before the answer arrived: nothing went wrong, so nothing alarming is said.
      if (failure.code === 'notFound') {
        this.notices.push({ title: t('memory.request.ended'), tone: 'neutral' });
      } else {
        this.notices.push({ title: t('memory.request.failed', { name: request.script.name }), description: failure.message, tone: 'danger' });
      }
    } finally {
      this.answering = false;
    }
  }

  /**
   * Starts hearing the backend's requests (those still waiting arrive again at once). Returns the function that stops
   * listening. The backend keeps one listener, so a later `start` takes over from this one.
   */
  start(): () => void {
    let listening = true;

    this.transport
      .invoke('memory_script_requests_listen', {
        onRequest: (event: ScriptRequestEvent) => {
          if (!listening) {
            return;
          }

          if (event.type === 'request') {
            this.push(event.request);
          } else {
            this.ended(event.requestId);
          }
        },
      })
      .catch(() => {});

    return () => {
      listening = false;
    };
  }

  /** Forgets every request, answered ones included. */
  clear(): void {
    this.answered = [];
    saveAnswered(this.answered);
    this.timers.forEach((timer) => clearTimeout(timer));
    this.timers.clear();
    this.queue = [];
    this.answering = false;
  }

  private remove(requestId: string): void {
    this.answered = [...this.answered.filter((id) => id !== requestId), requestId].slice(-REMEMBERED_ANSWERS);
    saveAnswered(this.answered);

    clearTimeout(this.timers.get(requestId));
    this.timers.delete(requestId);
    this.queue = this.queue.filter((waiting) => waiting.request.requestId !== requestId);
  }
}

export const scriptRequests = new ScriptRequestsStore(ipc, toasts);
