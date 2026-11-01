import { t, type MessageKey } from '$lib/i18n/i18n.svelte';
import type { KnowledgeCheck } from '$lib/ipc/bindings/KnowledgeCheck';
import type { KnowledgeProblem } from '$lib/ipc/bindings/KnowledgeProblem';
import type { KnowledgeRemote } from '$lib/ipc/bindings/KnowledgeRemote';
import type { MemoryNotesRouting } from '$lib/ipc/bindings/MemoryNotesRouting';
import type { MemoryRemoteKind } from '$lib/ipc/bindings/MemoryRemoteKind';
import type { DocumentRouting } from '$lib/utils/knowledge-routing';

import { MEMORY_REMOTE } from './memory-remote';
import type { ConnectionTested, SavedConnection } from './remote-connection.svelte';
import type { RemoteDatabaseConfig, TeamDatabaseCopy } from './remote-database';
import { kindOf, type RoutingConfirmCopy, type RoutingEffect, type RoutingEffectKind } from './routing-change';

/** The team databases Harness Knowledge can use. Amazon DocumentDB is not one of them. */
export type KnowledgeKind = Exclude<MemoryRemoteKind, 'documentdb'>;

/** The databases are the ones of Harness Memory without DocumentDB, with the words of Knowledge where they name what is stored. */
export const KNOWLEDGE_REMOTE: RemoteDatabaseConfig<KnowledgeKind> = {
  kinds: ['postgres', 'mongodb', 'libsql'],
  defaultKind: 'postgres',
  schemes: { postgres: MEMORY_REMOTE.schemes.postgres, mongodb: MEMORY_REMOTE.schemes.mongodb, libsql: MEMORY_REMOTE.schemes.libsql },
  tokenKinds: ['libsql'],
  kindLabels: { postgres: MEMORY_REMOTE.kindLabels.postgres, mongodb: MEMORY_REMOTE.kindLabels.mongodb, libsql: MEMORY_REMOTE.kindLabels.libsql },
  urlPlaceholders: {
    postgres: MEMORY_REMOTE.urlPlaceholders.postgres,
    mongodb: 'knowledge.settings.url.placeholder.mongodb',
    libsql: 'knowledge.settings.url.placeholder.libsql',
  },
  urlProblems: { postgres: MEMORY_REMOTE.urlProblems.postgres, mongodb: MEMORY_REMOTE.urlProblems.mongodb, libsql: MEMORY_REMOTE.urlProblems.libsql },
  copy: {
    ...MEMORY_REMOTE.copy,
    databaseDescription: 'knowledge.settings.database.description',
    restart: 'knowledge.settings.restart',
    saved: 'knowledge.settings.saved',
  },
};

/** The rows that switch the team database on and off. */
export const KNOWLEDGE_TEAM_DATABASE: TeamDatabaseCopy = {
  title: 'knowledge.settings.remote.title',
  description: 'knowledge.settings.remote.description',
  loadFailed: 'knowledge.settings.remote.loadFailed',
  offTitle: 'knowledge.settings.remote.off.title',
  offBody: 'knowledge.settings.remote.off.body',
  offCancel: 'settings.memoryRemote.off.cancel',
  offConfirm: 'settings.memoryRemote.off.confirm',
};

/** What each thing a database can lack says. `other` has no sentence of its own: the server's line stands in. */
const PROBLEMS: Record<Exclude<KnowledgeProblem, 'other'>, MessageKey> = {
  noMongot: 'knowledge.settings.problem.noMongot',
  noPgvector: 'knowledge.settings.problem.noPgvector',
  pgvectorNoPrivilege: 'knowledge.settings.problem.pgvectorNoPrivilege',
  pgvectorTooOld: 'knowledge.settings.problem.pgvectorTooOld',
  libsqlNoVector: 'knowledge.settings.problem.libsqlNoVector',
  noFts5: 'knowledge.settings.problem.noFts5',
  indexBuilding: 'knowledge.settings.problem.indexBuilding',
  unreachable: 'knowledge.settings.problem.unreachable',
  auth: 'knowledge.settings.problem.auth',
};

/** What a database lacks, in the app's language: what and how to fix it. `other` has no sentence of its own, the server's line stands in. */
export function problemText(problem: KnowledgeProblem, message: string | null): string {
  if (problem !== 'other') {
    return t(PROBLEMS[problem]);
  }

  return t('knowledge.settings.problem.unknown', { message: message ?? t('knowledge.settings.problem.noMessage') });
}

/** What a check found, in the app's language. A database that has everything says so, one that lacks something says what and how to fix it. */
export function checkText(check: KnowledgeCheck): string {
  if (check.ok) {
    return t('knowledge.settings.test.ok');
  }

  // A check that found no problem to name still failed: the server's line stands in.
  return problemText(check.problem ?? 'other', check.message);
}

/** The answer of a connection test, as the form shows it. */
export function toTested(check: KnowledgeCheck): ConnectionTested {
  return { ok: check.ok, message: checkText(check) };
}

/** The kind of a saved database, when Harness Knowledge supports it. */
function supportedKind(kind: MemoryRemoteKind | null): KnowledgeKind | null {
  return kind === null || kind === 'documentdb' ? null : kind;
}

/** What the backend has saved, as the connection form reads it. */
export function savedConnection(remote: KnowledgeRemote): SavedConnection<KnowledgeKind> {
  return { ...remote, kind: supportedKind(remote.kind) };
}

/** The two kinds of documents whose place the user chooses. */
export type DocumentScope = 'repositoryDocuments' | 'globalDocuments';

const SCOPES: readonly DocumentScope[] = ['repositoryDocuments', 'globalDocuments'];

/** The line under a row's control, per place. */
export function documentHints(): Record<MemoryNotesRouting, string> {
  return {
    remote: t('knowledge.settings.hint.remote'),
    local: t('knowledge.settings.hint.local'),
    remoteLocal: t('knowledge.settings.hint.remoteLocal'),
  };
}

/** What saving `to` instead of `from` changes in what the user sees, per kind of document that moves. */
export function documentEffects(from: DocumentRouting, to: DocumentRouting): RoutingEffect<DocumentScope>[] {
  return SCOPES.flatMap((scope) => {
    const kind = kindOf(from[scope], to[scope]);

    return kind === null ? [] : [{ scope, kind, count: null }];
  });
}

const EFFECT_TEXTS: Record<RoutingEffectKind, MessageKey> = {
  hideTeam: 'knowledge.settings.change.hideTeam',
  showTeam: 'knowledge.settings.change.showTeam',
  saveHere: 'knowledge.settings.change.saveHere',
  hideLocal: 'knowledge.settings.change.hideLocal',
};

/** The words of the warning shown before a change of where documents are kept. */
export function documentConfirmCopy(): RoutingConfirmCopy {
  return {
    title: t('knowledge.settings.change.title'),
    description: t('knowledge.settings.change.description'),
    describe: (effect) => ({
      title: t(effect.scope === 'repositoryDocuments' ? 'knowledge.settings.projectDocs.title' : 'knowledge.settings.globalDocs.title'),
      text: t(EFFECT_TEXTS[effect.kind]),
    }),
  };
}
