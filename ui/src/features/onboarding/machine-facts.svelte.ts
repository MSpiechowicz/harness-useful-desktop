import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';
import type { ProjectInfo } from '$lib/ipc/bindings/ProjectInfo';
import { ipc } from '$lib/ipc/client';

/** One fact about a harness on this machine: still being read, read, or not known (it could not be read, or does not apply). */
export type Fact<T> = { status: 'loading' } | { status: 'ready'; value: T } | { status: 'unknown' };

export interface SessionsFact {
  /** The project whose sessions were counted. */
  project: string;
  count: number;
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

async function readSessions(harness: HarnessId, project: ProjectInfo | null): Promise<Fact<SessionsFact>> {
  if (project === null) {
    return { status: 'unknown' };
  }

  // The backend lists a project's sessions in one answer (newest first) and has no total, so this counts that one project's.
  const sessions = (await ipc.invoke('sessions_list', { harness, project: project.path })).filter((session) => !session.emptyCopy);

  return { status: 'ready', value: { project: project.name, count: sessions.length, lastUsed: sessions[0]?.updatedAt ?? null } };
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

async function settle<T>(read: Promise<Fact<T>>): Promise<Fact<T>> {
  try {
    return await read;
  } catch {
    return { status: 'unknown' };
  }
}

/**
 * What the picker step shows about each harness on this machine, read when it is first looked at and kept for the
 * step: moving between rows asks nothing again. A fact that cannot be read stays unknown, and the panel leaves it out.
 */
export class MachineFactsCache {
  private facts = $state<Partial<Record<HarnessId, MachineFacts>>>({});

  of(id: HarnessId): MachineFacts | undefined {
    return this.facts[id];
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
