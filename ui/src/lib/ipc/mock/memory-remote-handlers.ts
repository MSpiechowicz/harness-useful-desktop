import type { MemoryNotesRouting } from '../bindings/MemoryNotesRouting';
import type { MemoryRemote } from '../bindings/MemoryRemote';
import type { MemoryRemoteKind } from '../bindings/MemoryRemoteKind';
import type { CommandArgs } from '../commands';
import { IpcFailure } from '../transport';
import { notesAuthorFilterListed, notesAuthorsListed } from './authors';
import { embedderScenarioFromLocation } from './memory-embedder-handlers';
import { notesLangListed, notesUsefulListed } from './memory-notes-handlers';
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
export const CHECK_MS = import.meta.env.MODE === 'test' ? 0 : 900;

export const KIND_NAMES: Record<MemoryRemoteKind, string> = { postgres: 'PostgreSQL', libsql: 'libSQL', mongodb: 'MongoDB', documentdb: 'Amazon DocumentDB' };

export const SCHEMES: Record<MemoryRemoteKind, string[]> = {
  postgres: ['postgres', 'postgresql'],
  libsql: ['libsql', 'https', 'http'],
  mongodb: ['mongodb', 'mongodb+srv'],
  documentdb: ['mongodb', 'mongodb+srv'],
};

/** What the mock "file" holds; the raw URL and token never leave this module. */
interface Saved {
  url: string | null;
  token: string | null;
  repositoryNotes: MemoryNotesRouting;
  globalNotes: MemoryNotesRouting;
  scripts: MemoryNotesRouting;
  trustTeamScripts: boolean;
  shareHooks: boolean;
  trustTeamHooks: boolean;
  shareRoutines: boolean;
  trustTeamRoutines: boolean;
  /** Why the file must not be overwritten (keys Harness Memory does not know). */
  blocked: string | null;
}

function initial(scenario: RemoteScenario): Saved {
  const base: Saved = { url: null, token: null, repositoryNotes: 'remote', globalNotes: 'remote', scripts: 'remote', trustTeamScripts: false, shareHooks: false, trustTeamHooks: false, shareRoutines: false, trustTeamRoutines: false, blocked: null };

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

/** Whether the URL's scheme is one the database accepts. MongoDB and DocumentDB share theirs. */
export function schemeFits(url: string, kind: MemoryRemoteKind): boolean {
  const scheme = url.split('://')[0]?.toLowerCase() ?? '';

  return url.includes('://') && SCHEMES[kind].includes(scheme);
}

/** The kind a saved URL stands for: a mongodb URL on an AWS DocumentDB host is DocumentDB. */
export function kindOf(url: string): MemoryRemoteKind | null {
  if (schemeFits(url, 'documentdb') && url.toLowerCase().includes('.docdb.amazonaws.com')) {
    return 'documentdb';
  }

  return (Object.keys(SCHEMES) as MemoryRemoteKind[]).find((kind) => kind !== 'documentdb' && schemeFits(url, kind)) ?? null;
}

/** `scheme://user@host:port/path`: no password, query or fragment. */
export function redact(url: string): string {
  const [scheme, rest = ''] = url.split('://');
  const at = rest.lastIndexOf('@');
  const user = at >= 0 ? rest.slice(0, at).split(':')[0] : '';
  const location = (at >= 0 ? rest.slice(at + 1) : rest).split(/[?#]/)[0];

  return user ? `${scheme}://${user}@${location}` : `${scheme}://${location}`;
}

export function invalid(message: string): IpcFailure {
  return IpcFailure.from({ code: 'invalidInput', message });
}

/** The URL as the real backend validates it (the error never quotes it). */
export function validUrl(raw: string, kind: MemoryRemoteKind): string {
  const url = raw.trim();

  if (!url) {
    throw invalid('enter the database URL');
  }

  if (url.length > 4096 || /\p{Cc}/u.test(url)) {
    throw invalid('the database URL is longer than 4 KiB or contains control characters');
  }

  if (!schemeFits(url, kind)) {
    throw invalid(`a ${KIND_NAMES[kind]} URL must start with ${SCHEMES[kind].map((scheme) => `${scheme}://`).join(', ')}`);
  }

  const rest = url.split('://')[1] ?? '';
  if (kind === 'libsql' && (/[?#]/.test(rest) || (rest.split('/')[0] ?? '').includes('@'))) {
    throw invalid('a libSQL URL takes no user, password, query or fragment; enter the token in its own field');
  }

  return url;
}

/** The URL a save keeps: the typed one when it is valid, otherwise the saved one while it fits the database. */
export function urlToSave(saved: string | null, typed: string | null, kind: MemoryRemoteKind): string {
  if (typed !== null) {
    return validUrl(typed, kind);
  }

  if (!saved || !schemeFits(saved, kind)) {
    throw invalid(saved ? `the saved URL is not a ${KIND_NAMES[kind]} URL; enter the new one` : 'enter the database URL');
  }

  return saved;
}

/** The token a save keeps: only libSQL has one, and a null keeps the saved token. */
export function tokenToSave(saved: string | null, typed: string | null, kind: MemoryRemoteKind): string | null {
  if (kind !== 'libsql') {
    return null;
  }

  return typed === null ? saved : typed || null;
}

/** The URL a test tries (the typed one, or the saved one), or why there is none to try. */
export function urlToTest(saved: string | null, typed: string | null, kind: MemoryRemoteKind): { url: string } | { problem: string } {
  let url: string;

  try {
    url = typed === null ? (saved ?? '') : validUrl(typed, kind);
  } catch (error) {
    return { problem: error instanceof IpcFailure ? error.message : String(error) };
  }

  if (!url) {
    return { problem: 'no database URL is saved; enter one to test it' };
  }

  return schemeFits(url, kind) ? { url } : { problem: `the saved URL is not a ${KIND_NAMES[kind]} URL; enter one to test it` };
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
      // Harness Memory keeps team hooks in the same releases that keep scripts, in the previews.
      hooksSupported: scriptsSupported(),
      shareHooks: file.shareHooks,
      trustTeamHooks: file.trustTeamHooks,
      // Harness Memory keeps team routines in the same releases, in the previews.
      routinesSupported: scriptsSupported(),
      shareRoutines: file.shareRoutines,
      trustTeamRoutines: file.trustTeamRoutines,
      // The mock stands in for a current Harness Memory.
      documentdbSupported: true,
      // Likewise for saving notes on this computer next to the team database, hiding team notes and copying.
      teamAndLocalSupported: true,
      scriptsTeamAndLocalSupported: true,
      // And for save guards: the Memory button, held saves and the Settings cards.
      saveGuardsSupported: true,
      saveOptInSupported: true,
      // Vector search and the English notes setting come in one release: `?embedder=old` is a Harness Memory without either, `?lang=old` one without note languages.
      embedderSupported: embedderScenarioFromLocation() !== 'old',
      settingsSupported: embedderScenarioFromLocation() !== 'old',
      notesLangSupported: notesLangListed(),
      // `?useful=old` is a Harness Memory that does not count how useful a note was.
      notesUsefulSupported: notesUsefulListed(),
      // `?authors=old` is one that does not tell who wrote a note or a script.
      notesAuthorsSupported: notesAuthorsListed(),
      // `?authorFilter=old` is one that tells who wrote them but cannot filter or sort by it.
      notesAuthorFilterSupported: notesAuthorFilterListed(),
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
      saved = { ...file, url: null, token: null, repositoryNotes: 'remote', globalNotes: 'remote', scripts: 'remote', trustTeamScripts: false, shareHooks: false, trustTeamHooks: false, shareRoutines: false, trustTeamRoutines: false };
      return view();
    }

    if (!args.kind) {
      throw invalid('pick the kind of remote database');
    }

    const url = urlToSave(file.url, args.url, args.kind);

    const token = tokenToSave(file.token, args.token, args.kind);
    // Team hooks are written only while the server keeps them, and the trust only while they are shared. A null keeps the saved choice.
    const shareHooks = scriptsSupported() ? (args.shareHooks ?? file.shareHooks) : file.shareHooks;
    const trustTeamHooks = shareHooks && (scriptsSupported() ? (args.trustTeamHooks ?? file.trustTeamHooks) : file.trustTeamHooks);
    const shareRoutines = scriptsSupported() ? (args.shareRoutines ?? file.shareRoutines) : file.shareRoutines;
    const trustTeamRoutines = shareRoutines && (scriptsSupported() ? (args.trustTeamRoutines ?? file.trustTeamRoutines) : file.trustTeamRoutines);
    saved = { url, token, repositoryNotes: args.repositoryNotes, globalNotes: args.globalNotes, scripts: scriptsSupported() ? args.scripts : 'remote', trustTeamScripts: scriptsSupported() ? args.trustTeamScripts : false, shareHooks, trustTeamHooks, shareRoutines, trustTeamRoutines, blocked: null };

    return view();
  }

  async function test(args: CommandArgs<'memory_remote_test'>) {
    await new Promise((resolve) => setTimeout(resolve, CHECK_MS));

    const found = urlToTest(current().url, args.url, args.kind);

    if ('problem' in found) {
      return { ok: false, message: found.problem };
    }

    const { url } = found;

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
