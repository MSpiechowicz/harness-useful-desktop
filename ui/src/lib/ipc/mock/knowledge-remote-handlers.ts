import type { KnowledgeCheck } from '../bindings/KnowledgeCheck';
import type { KnowledgeProblem } from '../bindings/KnowledgeProblem';
import type { KnowledgeRemote } from '../bindings/KnowledgeRemote';
import type { MemoryRemoteKind } from '../bindings/MemoryRemoteKind';
import type { CommandArgs } from '../commands';
import { CHECK_MS, invalid, KIND_NAMES, kindOf, redact, tokenToSave, urlToSave, urlToTest } from './memory-remote-handlers';
import type { DocumentRouting } from '$lib/utils/knowledge-routing';

import type { Handlers } from './types';

/** Starting states of the Knowledge team database settings, picked with `?kremote=` in the preview. */
export const KNOWLEDGE_REMOTE_SCENARIOS = ['none', 'postgres', 'libsql', 'problem'] as const;
export type KnowledgeRemoteScenario = (typeof KNOWLEDGE_REMOTE_SCENARIOS)[number];

export function knowledgeRemoteScenarioFromLocation(search: string = globalThis.location?.search ?? ''): KnowledgeRemoteScenario {
  const name = new URLSearchParams(search).get('kremote');

  return KNOWLEDGE_REMOTE_SCENARIOS.find((scenario) => scenario === name) ?? 'none';
}

const CONFIG_PATH = '~/.config/harness-knowledge/config.toml';

/** What a test finds in a URL with this word in it, as the server's `check` would report a database that lacks it. */
const PROBLEM_WORDS: [string, KnowledgeProblem][] = [
  ['nomongot', 'noMongot'],
  ['nopgvector', 'noPgvector'],
  ['pgvectorprivilege', 'pgvectorNoPrivilege'],
  ['pgvectorold', 'pgvectorTooOld'],
  ['nolibsqlvector', 'libsqlNoVector'],
  ['nofts5', 'noFts5'],
  ['indexbuilding', 'indexBuilding'],
  ['badauth', 'auth'],
];

interface Saved {
  url: string | null;
  token: string | null;
  blocked: string | null;
}

function initial(scenario: KnowledgeRemoteScenario): Saved {
  switch (scenario) {
    case 'postgres':
      return { url: 'postgres://knowledge:hunter2@pg.team-ok.example.com:5432/harness_knowledge?sslmode=require', token: null, blocked: null };
    case 'libsql':
      return { url: 'libsql://team-knowledge-ok.turso.io', token: 'mock-token', blocked: null };
    case 'problem':
      return { url: 'mongodb+srv://knowledge:hunter2@cluster0.example.net/harness_knowledge', token: null, blocked: 'the file has keys Harness Knowledge does not know (databse_token). Remove them by hand first' };
    case 'none':
      return { url: null, token: null, blocked: null };
  }
}

/** The kinds Harness Knowledge supports: a MongoDB URL on a DocumentDB host is a MongoDB URL for it. */
function knowledgeKindOf(url: string): MemoryRemoteKind | null {
  const kind = kindOf(url);

  return kind === 'documentdb' ? 'mongodb' : kind;
}

function check(url: string, kind: MemoryRemoteKind): KnowledgeCheck {
  const lower = url.toLowerCase();
  const found = PROBLEM_WORDS.find(([word]) => lower.includes(word));
  const backend = kind === 'documentdb' ? 'mongodb' : kind;

  if (found) {
    return { ok: false, backend, problem: found[1], message: `${KIND_NAMES[kind]} at ${redact(url)} is missing something Harness Knowledge needs`, capabilities: { keyword: true, vector: false, vectorIndex: found[1] === 'indexBuilding' ? 'building' : 'missing' }, model: null };
  }

  if (lower.includes('ok')) {
    return { ok: true, backend, problem: null, message: `ok: ${KIND_NAMES[kind]} at ${redact(url)} (schema 1)`, capabilities: { keyword: true, vector: true, vectorIndex: 'ready' }, model: 'multilingual-e5-small' };
  }

  return { ok: false, backend, problem: 'unreachable', message: `cannot connect to ${redact(url)}: connection refused`, capabilities: null, model: null };
}

/** Team database settings kept in memory: `knowledge_remote_get/set/test` like the real backend, without a real server. `hasEmbedder` is whether the installed Harness Knowledge lists an embedder (none installed, or an old one, does not). */
export function knowledgeRemoteHandlers(scenario: () => KnowledgeRemoteScenario = knowledgeRemoteScenarioFromLocation, hasEmbedder: () => boolean = () => true, routing: DocumentRouting = { repositoryDocuments: 'remote', globalDocuments: 'remote' }): Pick<Handlers, 'knowledge_remote_get' | 'knowledge_remote_set' | 'knowledge_remote_test'> {
  let saved: Saved | null = null;
  const routes = { ...routing };
  const current = (): Saved => (saved ??= initial(scenario()));

  function view(): KnowledgeRemote {
    const file = current();

    return {
      enabled: file.url !== null,
      kind: file.url ? knowledgeKindOf(file.url) : null,
      urlRedacted: file.url ? redact(file.url) : null,
      hasToken: file.token !== null,
      embedderSupported: hasEmbedder(),
      repositoryDocuments: routes.repositoryDocuments,
      globalDocuments: routes.globalDocuments,
      routingSupported: hasEmbedder(),
      configPath: CONFIG_PATH,
      problem: file.blocked,
    };
  }

  function set(args: CommandArgs<'knowledge_remote_set'>): KnowledgeRemote {
    const file = current();

    if (file.blocked) {
      throw invalid(`${CONFIG_PATH} is not changed: ${file.blocked}`);
    }

    if (!args.enabled) {
      saved = { url: null, token: null, blocked: null };

      return view();
    }

    if (!args.kind) {
      throw invalid('pick the kind of remote database');
    }

    routes.repositoryDocuments = args.repositoryDocuments ?? routes.repositoryDocuments;
    routes.globalDocuments = args.globalDocuments ?? routes.globalDocuments;

    const url = urlToSave(file.url, args.url, args.kind);

    const token = tokenToSave(file.token, args.token, args.kind);
    saved = { url, token, blocked: null };

    return view();
  }

  async function test(args: CommandArgs<'knowledge_remote_test'>): Promise<KnowledgeCheck> {
    await new Promise((resolve) => setTimeout(resolve, CHECK_MS));

    const found = urlToTest(current().url, args.url, args.kind);

    if ('problem' in found) {
      return { ok: false, backend: null, problem: 'other', message: found.problem, capabilities: null, model: null };
    }

    return check(found.url, args.kind);
  }

  return {
    knowledge_remote_get: () => view(),
    knowledge_remote_set: set,
    knowledge_remote_test: test,
  };
}
