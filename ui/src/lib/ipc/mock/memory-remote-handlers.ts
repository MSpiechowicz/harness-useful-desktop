import type { MemoryNotesTarget } from '../bindings/MemoryNotesTarget';
import type { MemoryRemote } from '../bindings/MemoryRemote';
import type { MemoryRemoteKind } from '../bindings/MemoryRemoteKind';
import type { CommandArgs } from '../commands';
import { IpcFailure } from '../transport';
import { scriptsSupported } from './memory-scripts-handlers';
import type { Handlers } from './types';

/** Starting states of the team database settings, picked with `?remote=` in the preview. */
export const REMOTE_SCENARIOS = ['none', 'postgres', 'libsql', 'problem'] as const;
export type RemoteScenario = (typeof REMOTE_SCENARIOS)[number];

export function remoteScenarioFromLocation(search: string = globalThis.location?.search ?? ''): RemoteScenario {
  const name = new URLSearchParams(search).get('remote');

  return REMOTE_SCENARIOS.find((scenario) => scenario === name) ?? 'none';
}

const CONFIG_PATH = '~/.config/harness-memory/config.toml';

/** How long a mock `harness-memory check` takes; tests do not wait. */
const CHECK_MS = import.meta.env.MODE === 'test' ? 0 : 900;

const KIND_NAMES: Record<MemoryRemoteKind, string> = { postgres: 'PostgreSQL', libsql: 'libSQL', mongodb: 'MongoDB' };

const SCHEMES: Record<MemoryRemoteKind, string[]> = {
  postgres: ['postgres', 'postgresql'],
  libsql: ['libsql', 'https', 'http'],
  mongodb: ['mongodb', 'mongodb+srv'],
};

/** What the mock "file" holds; the raw URL and token never leave this module. */
interface Saved {
  url: string | null;
  token: string | null;
  repositoryNotes: MemoryNotesTarget;
  globalNotes: MemoryNotesTarget;
  scripts: MemoryNotesTarget;
  trustTeamScripts: boolean;
  /** Why the file must not be overwritten (keys Harness Memory does not know). */
  blocked: string | null;
}

function initial(scenario: RemoteScenario): Saved {
  const base: Saved = { url: null, token: null, repositoryNotes: 'remote', globalNotes: 'remote', scripts: 'remote', trustTeamScripts: false, blocked: null };

  switch (scenario) {
    case 'postgres':
      return { ...base, url: 'postgres://memory:hunter2@pg.team-ok.example.com:5432/harness_memory?sslmode=require' };
    case 'libsql':
      return { ...base, url: 'libsql://team-memory-ok.turso.io', token: 'mock-libsql-token', repositoryNotes: 'local' };
    case 'problem':
      return { ...base, url: 'mongodb+srv://memory:hunter2@cluster0.example.net/harness_memory', blocked: 'the file has keys Harness Memory does not know (databse_token); remove them by hand first' };
    case 'none':
      return base;
  }
}

function kindOf(url: string): MemoryRemoteKind | null {
  const scheme = url.split('://')[0]?.toLowerCase() ?? '';

  return (Object.keys(SCHEMES) as MemoryRemoteKind[]).find((kind) => url.includes('://') && SCHEMES[kind].includes(scheme)) ?? null;
}

/** `scheme://user@host:port/path`: no password, query or fragment. */
function redact(url: string): string {
  const [scheme, rest = ''] = url.split('://');
  const at = rest.lastIndexOf('@');
  const user = at >= 0 ? rest.slice(0, at).split(':')[0] : '';
  const location = (at >= 0 ? rest.slice(at + 1) : rest).split(/[?#]/)[0];

  return user ? `${scheme}://${user}@${location}` : `${scheme}://${location}`;
}

function invalid(message: string): IpcFailure {
  return IpcFailure.from({ code: 'invalidInput', message });
}

/** The URL as the real backend validates it (the error never quotes it). */
function validUrl(raw: string, kind: MemoryRemoteKind): string {
  const url = raw.trim();

  if (!url) {
    throw invalid('enter the database URL');
  }

  if (url.length > 4096 || /\p{Cc}/u.test(url)) {
    throw invalid('the database URL is longer than 4 KiB or contains control characters');
  }

  if (kindOf(url) !== kind) {
    throw invalid(`a ${KIND_NAMES[kind]} URL must start with ${SCHEMES[kind].map((scheme) => `${scheme}://`).join(', ')}`);
  }

  const rest = url.split('://')[1] ?? '';
  if (kind === 'libsql' && (/[?#]/.test(rest) || (rest.split('/')[0] ?? '').includes('@'))) {
    throw invalid('a libSQL URL takes no user, password, query or fragment; enter the token in its own field');
  }

  return url;
}

/** Team database settings kept in memory: `memory_remote_get/set/test` like the real backend, without a real server. */
export function memoryRemoteHandlers(scenario: () => RemoteScenario = remoteScenarioFromLocation): Pick<Handlers, 'memory_remote_get' | 'memory_remote_set' | 'memory_remote_test'> {
  let saved: Saved | null = null;
  const current = (): Saved => (saved ??= initial(scenario()));

  function view(): MemoryRemote {
    const file = current();

    return {
      enabled: file.url !== null,
      kind: file.url ? kindOf(file.url) : null,
      urlRedacted: file.url ? redact(file.url) : null,
      hasToken: file.token !== null,
      repositoryNotes: file.repositoryNotes,
      globalNotes: file.globalNotes,
      scripts: file.scripts,
      scriptsSupported: scriptsSupported(),
      trustTeamScripts: file.trustTeamScripts,
      trustTeamScriptsSupported: scriptsSupported(),
      configPath: CONFIG_PATH,
      problem: file.blocked,
    };
  }

  function set(args: CommandArgs<'memory_remote_set'>): MemoryRemote {
    const file = current();

    if (file.blocked) {
      throw invalid(`${CONFIG_PATH} is not changed: ${file.blocked}`);
    }

    if (!args.enabled) {
      saved = { ...file, url: null, token: null, repositoryNotes: 'remote', globalNotes: 'remote', scripts: 'remote', trustTeamScripts: false };
      return view();
    }

    if (!args.kind) {
      throw invalid('pick the kind of team database');
    }

    let url = file.url;
    if (args.url !== null) {
      url = validUrl(args.url, args.kind);
    } else if (!url || kindOf(url) !== args.kind) {
      throw invalid(url ? `the saved URL is not a ${KIND_NAMES[args.kind]} URL; enter the new one` : 'enter the database URL');
    }

    const token = args.kind !== 'libsql' ? null : args.token === null ? file.token : args.token.trim() || null;
    saved = { url, token, repositoryNotes: args.repositoryNotes, globalNotes: args.globalNotes, scripts: scriptsSupported() ? args.scripts : 'remote', trustTeamScripts: scriptsSupported() ? args.trustTeamScripts : false, blocked: null };

    return view();
  }

  async function test(args: CommandArgs<'memory_remote_test'>) {
    await new Promise((resolve) => setTimeout(resolve, CHECK_MS));

    let url: string;
    try {
      url = args.url === null ? (current().url ?? '') : validUrl(args.url, args.kind);
    } catch (error) {
      return { ok: false, message: error instanceof IpcFailure ? error.message : String(error) };
    }

    if (!url) {
      return { ok: false, message: 'no database URL is saved; enter one to test it' };
    }

    if (kindOf(url) !== args.kind) {
      return { ok: false, message: `the saved URL is not a ${KIND_NAMES[args.kind]} URL; enter one to test it` };
    }

    if (url.includes('ok')) {
      return { ok: true, message: `ok: ${KIND_NAMES[args.kind]} at ${redact(url)} (schema 3)` };
    }

    return { ok: false, message: `error: cannot connect to ${redact(url)}: connection refused` };
  }

  return {
    memory_remote_get: () => view(),
    memory_remote_set: set,
    memory_remote_test: test,
  };
}
