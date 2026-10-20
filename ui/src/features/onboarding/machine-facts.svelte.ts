import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';
import type { ProjectInfo } from '$lib/ipc/bindings/ProjectInfo';
import { ipc } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/transport';

/** One fact about a harness on this machine: still being read, read, or not known (it could not be read, or does not apply). */
export type Fact<T> = { status: 'loading' } | { status: 'ready'; value: T } | { status: 'unknown' };

export interface SessionsFact {
  /** The project whose sessions were counted, or null when the count covers every project on this machine. */
  project: string | null;
  count: number;
  /** Whether the count stopped at a limit, so there are at least this many. */
  capped: boolean;
  /** When the newest session was last written (RFC 3339), or null when there is none. */
  lastUsed: string | null;
}

/** What the harness has installed: only the counts the harness supports are set. */
export interface ExtensionsFact {
  plugins: number | null;
  packages: number | null;
  mcpServers: number | null;
}

export interface MachineFacts {
  sessions: Fact<SessionsFact>;
  extensions: Fact<ExtensionsFact>;
}

/** The project whose sessions stand for the harness's use: the open one, else the newest one that still exists. */
export function projectToCount(active: ProjectInfo | null, recents: ProjectInfo[]): ProjectInfo | null {
  return active ?? recents.find((recent) => recent.exists) ?? null;
}

/** What the harness's sessions are across every project on this machine, or null when the harness cannot tell. */
async function readTotals(harness: HarnessId): Promise<SessionsFact | null> {
  try {
    const { count, lastUsed, capped } = await ipc.invoke('harness_session_totals', { harness });

    return { project: null, count, capped, lastUsed };
  } catch (error) {
    // Only a harness that cannot total its sessions falls back to one project. Any other failure leaves the fact unknown.
    if (IpcFailure.from(error).code === 'unsupported') {
      return null;
    }

    throw error;
  }
}

async function readSessions(harness: HarnessId, project: ProjectInfo | null): Promise<Fact<SessionsFact>> {
  const totals = await readTotals(harness);
  // A walk cut off before it counted anything tells nothing, so it is unknown rather than "0+".
  if (totals !== null && totals.capped && totals.count === 0) {
    return { status: 'unknown' };
  }

  if (totals !== null) {
    return { status: 'ready', value: totals };
  }

  if (project === null) {
    return { status: 'unknown' };
  }

  // The backend lists a project's sessions in one answer (newest first) and has no total, so this counts that one project's.
  const sessions = (await ipc.invoke('sessions_list', { harness, project: project.path })).filter((session) => !session.emptyCopy);

  return { status: 'ready', value: { project: project.name, count: sessions.length, capped: false, lastUsed: sessions[0]?.updatedAt ?? null } };
}

async function readExtensions(harness: HarnessSummary): Promise<Fact<ExtensionsFact>> {
  const { capabilities } = harness;
  if (!capabilities.plugins && !capabilities.packages && !capabilities.mcp) {
    return { status: 'unknown' };
  }

  const view = await ipc.invoke('extensions_view', { harness: harness.info.id, project: null });

  return {
    status: 'ready',
    value: {
      plugins: capabilities.plugins ? view.plugins.length : null,
      packages: capabilities.packages ? view.packages.length : null,
      mcpServers: capabilities.mcp ? view.mcpServers.length : null,
    },
  };
}

/** The app cannot read the sessions of a harness it cannot chat with, and a missing one has nothing to read. */
export function readable(harness: HarnessSummary): boolean {
  return harness.detection.installed && harness.capabilities.chat;
}

async function settle<T>(read: Promise<Fact<T>>): Promise<Fact<T>> {
  try {
    return await read;
  } catch {
    return { status: 'unknown' };
  }
}

/**
 * What the picker step shows about each harness on this machine, read when it is first needed (the intro step reads
 * them all) and kept for the onboarding run: moving between rows or steps asks nothing again. A fact that cannot be read stays unknown, and the panel leaves it out.
 */
export class MachineFactsCache {
  private facts = $state<Partial<Record<HarnessId, MachineFacts>>>({});

  of(id: HarnessId): MachineFacts | undefined {
    return this.facts[id];
  }

  /** Forgets what was read, so the next onboarding run reads the machine again. */
  reset(): void {
    this.facts = {};
  }

  /** Reads every harness the app can chat with and that is installed, each once, without waiting for any of them. */
  prefetch(harnesses: HarnessSummary[], project: ProjectInfo | null): void {
    for (const harness of harnesses) {
      if (readable(harness)) {
        this.load(harness, project);
      }
    }
  }

  /** Reads the facts of `harness` unless they were already asked for. */
  load(harness: HarnessSummary, project: ProjectInfo | null): void {
    const id = harness.info.id;
    if (this.facts[id]) {
      return;
    }

    this.facts[id] = { sessions: { status: 'loading' }, extensions: { status: 'loading' } };

    void settle(readSessions(id, project)).then((sessions) => {
      this.facts[id] = { ...(this.facts[id] as MachineFacts), sessions };
    });
    void settle(readExtensions(harness)).then((extensions) => {
      this.facts[id] = { ...(this.facts[id] as MachineFacts), extensions };
    });
  }
}

/** The facts of this onboarding run: read as early as the harnesses are known, so the picker finds them ready. */
export const machineFacts = new MachineFactsCache();
