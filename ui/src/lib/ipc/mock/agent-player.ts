import type { AgentEnvelope } from '../bindings/AgentEnvelope';
import type { AgentEvent } from '../bindings/AgentEvent';
import type { UiOutcome } from '../bindings/UiOutcome';
import type { UiResponse } from '../bindings/UiResponse';
import type { EventSink } from '../transport';
import { fixtures, type StreamName } from './fixtures';
import { SUBAGENT_STEP_MS, subagentDemo } from './subagent-demo';

/** `ready`, `state` and `commandsUpdated` open every recorded stream; they belong to session start, not to a prompt. */
const START_EVENTS = new Set<AgentEvent['type']>(['ready', 'state', 'commandsUpdated']);

const DELTA_MS: readonly [number, number] = [15, 40];
const STEP_MS = 90;

/** Which recorded stream answers a prompt: keywords in the text pick the scenario. */
export function pickStream(prompt: string): StreamName | 'subagents' {
  if (/\bsubagents?\b/i.test(prompt)) {
    return 'subagents';
  }
  if (/\bapproval\b/i.test(prompt)) {
    return 'approval';
  }
  if (/\bcrash\b/i.test(prompt)) {
    return 'crash';
  }
  if (/\berror\b/i.test(prompt)) {
    return 'error';
  }
  if (/\b(ui|dialogs?)\b/i.test(prompt)) {
    return 'uiRequests';
  }

  return 'basic';
}

/** Makes ids unique per prompt so several turns in one session never collide. */
export function retag(event: AgentEvent, prefix: string): AgentEvent {
  switch (event.type) {
    case 'messageStart':
    case 'textDelta':
    case 'thinkingDelta':
      return { ...event, messageId: prefix + event.messageId };

    case 'messageEnd':
      return { ...event, messageId: prefix + event.messageId, message: retagMessage(event.message, prefix) };

    case 'toolStart':
    case 'toolUpdate':
    case 'toolEnd':
      return { ...event, toolCallId: prefix + event.toolCallId };

    case 'uiRequest':
      return { ...event, request: { ...event.request, id: prefix + event.request.id } };

    case 'uiCancel':
      return { ...event, requestId: prefix + event.requestId };

    case 'promptResult':
      return { ...event, requestId: event.requestId === null ? null : prefix + event.requestId };

    case 'subagent': {
      const { subagent } = event;

      return { ...event, subagent: { ...subagent, parentToolCallId: subagent.parentToolCallId === null ? null : prefix + subagent.parentToolCallId } };
    }

    default:
      return event;
  }
}

function retagMessage(message: Extract<AgentEvent, { type: 'messageEnd' }>['message'], prefix: string) {
  return {
    ...message,
    toolCallId: message.toolCallId === null ? null : prefix + message.toolCallId,
    blocks: message.blocks.map((block) => (block.type === 'toolCall' ? { ...block, id: prefix + block.id } : block)),
  };
}

/** The recorded events of a scenario for one prompt, without the session-start events. */
export function promptEvents(name: StreamName, prompt: string, prefix: string): AgentEvent[] {
  const recorded = fixtures.agent.streams[name];
  const first = recorded.findIndex((envelope) => !START_EVENTS.has(envelope.event.type));

  return recorded
    .slice(first === -1 ? recorded.length : first)
    .map((envelope) => retag(structuredClone(envelope.event), prefix))
    .map((event) => (event.type === 'messageEnd' && event.message.role === 'user' ? withPromptText(event, prompt) : event));
}

function withPromptText(event: Extract<AgentEvent, { type: 'messageEnd' }>, prompt: string): AgentEvent {
  return { ...event, message: { ...event.message, blocks: [{ type: 'text', text: prompt }] } };
}

function isDenial(outcome: UiOutcome | null): boolean {
  if (outcome === null || outcome.type === 'cancelled') {
    return true;
  }

  return outcome.type === 'value' ? /^deny/i.test(outcome.value) : !outcome.confirmed;
}

function delayFor(event: AgentEvent): number {
  if (event.type === 'textDelta' || event.type === 'thinkingDelta') {
    return DELTA_MS[0] + Math.random() * (DELTA_MS[1] - DELTA_MS[0]);
  }
  if (event.type === 'subagent' && event.subagent.status !== 'pending') {
    return SUBAGENT_STEP_MS;
  }

  return STEP_MS;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * One mock agent session: replays the recorded stream picked by each prompt, pauses at UI requests
 * until the webview answers, and queues steering / follow-up text like the real agent.
 */
export class MockAgentStream {
  private seq = 0;
  private run = 0;
  private closed = false;
  private playing = false;
  private waiter: { id: string; resolve: (outcome: UiOutcome | null) => void } | null = null;
  private queued: { steering: string[]; followUp: string[] } = { steering: [], followUp: [] };

  constructor(
    readonly key: string,
    private readonly sink: EventSink<AgentEnvelope>,
    /** Multiplier for every pause; 0 replays instantly (motion off, tests). */
    private readonly pace: () => number,
  ) {}

  get streaming(): boolean {
    return this.playing;
  }

  emit(event: AgentEvent): void {
    this.seq += 1;
    this.sink({ sessionKey: this.key, seq: this.seq, event });
  }

  /** Replays the scenario for `prompt`; a new prompt while one is playing is the caller's job to queue. */
  async play(prompt: string): Promise<void> {
    this.run += 1;
    const run = this.run;
    this.playing = true;

    const name = pickStream(prompt);
    const prefix = `p${run}-`;
    let events = name === 'subagents' ? subagentDemo(prompt).map((event) => retag(event, prefix)) : promptEvents(name, prompt, prefix);
    const denied = name === 'approval' ? promptEvents('approvalDenied', prompt, prefix) : null;

    for (let index = 0; index < events.length; index += 1) {
      await sleep(delayFor(events[index]) * this.pace());
      if (this.stopped(run)) {
        return;
      }

      const event = events[index];
      this.emit(event);

      if (event.type === 'exited') {
        this.close();

        return;
      }

      if (event.type === 'uiRequest') {
        const outcome = await this.awaitOutcome(event.request.id);
        if (this.stopped(run)) {
          return;
        }

        if (denied && isDenial(outcome)) {
          events = [...events.slice(0, index + 1), ...denied.slice(index + 1)];
        }
      }
    }

    this.playing = false;
    await this.playNextQueued(run);
  }

  /** Steering and follow-up text wait for the current turn, then run as the next prompt. */
  enqueue(kind: 'steering' | 'followUp', message: string): void {
    this.queued[kind].push(message);
    this.emitQueue();
  }

  /** Withdraws one waiting message, like OMP's `remove_queued_message`; false when it is no longer queued. */
  dequeue(kind: 'steering' | 'followUp', message: string): boolean {
    const index = this.queued[kind].indexOf(message);
    if (index === -1) {
      return false;
    }

    this.queued[kind].splice(index, 1);
    this.emitQueue();

    return true;
  }

  private emitQueue(): void {
    this.emit({ type: 'queueUpdate', steering: [...this.queued.steering], followUp: [...this.queued.followUp], liveSteered: 0 });
  }

  respond(response: UiResponse): void {
    if (this.waiter?.id === response.id) {
      this.waiter.resolve(response.outcome);
      this.waiter = null;
    }
  }

  /** Stops the current turn the way the real agent reports an abort. */
  abort(): void {
    this.run += 1;
    this.waiter?.resolve(null);
    this.waiter = null;

    if (this.playing) {
      this.playing = false;
      this.emit({ type: 'agentEnd' });
      this.emit({ type: 'promptResult', requestId: null, status: 'aborted', error: null, retryable: false });
      this.emit({ type: 'settled', aborted: true });
    }
  }

  close(): void {
    this.closed = true;
    this.run += 1;
    this.waiter?.resolve(null);
    this.waiter = null;
  }

  private stopped(run: number): boolean {
    return this.closed || run !== this.run;
  }

  private awaitOutcome(id: string): Promise<UiOutcome | null> {
    return new Promise((resolve) => {
      this.waiter = { id, resolve };
    });
  }

  private async playNextQueued(run: number): Promise<void> {
    const next = this.queued.steering.shift() ?? this.queued.followUp.shift();
    if (next === undefined || this.stopped(run)) {
      return;
    }

    this.emitQueue();
    await this.play(next);
  }
}
