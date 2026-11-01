import type { MemoryRemoteKind } from '$lib/ipc/bindings/MemoryRemoteKind';

import type { RemoteDatabaseConfig } from './remote-database';

/** The team databases Harness Memory can use. Only libSQL (Turso) takes a separate auth token. MongoDB and Amazon DocumentDB share the mongodb schemes. */
export const MEMORY_REMOTE: RemoteDatabaseConfig<MemoryRemoteKind> = {
  kinds: ['postgres', 'libsql', 'mongodb', 'documentdb'],
  defaultKind: 'postgres',
  schemes: {
    postgres: ['postgres://', 'postgresql://'],
    libsql: ['libsql://', 'https://', 'http://'],
    mongodb: ['mongodb://', 'mongodb+srv://'],
    documentdb: ['mongodb://', 'mongodb+srv://'],
  },
  tokenKinds: ['libsql'],
  kindLabels: {
    postgres: 'settings.memoryRemote.kind.postgres',
    libsql: 'settings.memoryRemote.kind.libsql',
    mongodb: 'settings.memoryRemote.kind.mongodb',
    documentdb: 'settings.memoryRemote.kind.documentdb',
  },
  urlPlaceholders: {
    postgres: 'settings.memoryRemote.url.placeholder.postgres',
    libsql: 'settings.memoryRemote.url.placeholder.libsql',
    mongodb: 'settings.memoryRemote.url.placeholder.mongodb',
    documentdb: 'settings.memoryRemote.url.placeholder.documentdb',
  },
  urlProblems: {
    postgres: 'settings.memoryRemote.url.invalid.postgres',
    libsql: 'settings.memoryRemote.url.invalid.libsql',
    mongodb: 'settings.memoryRemote.url.invalid.mongodb',
    documentdb: 'settings.memoryRemote.url.invalid.documentdb',
  },
  copy: {
    connectionHeading: 'settings.memoryRemote.section.connection',
    databaseTitle: 'settings.memoryRemote.database.title',
    databaseDescription: 'settings.memoryRemote.database.description',
    restart: 'settings.memoryRemote.restart',
    stored: 'settings.memoryRemote.stored',
    urlLabel: 'settings.memoryRemote.url.label',
    urlKeep: 'settings.memoryRemote.url.keep',
    urlSaved: 'settings.memoryRemote.url.saved',
    urlShow: 'settings.memoryRemote.url.show',
    urlHide: 'settings.memoryRemote.url.hide',
    tokenLabel: 'settings.memoryRemote.token.label',
    tokenKeep: 'settings.memoryRemote.token.keep',
    tokenPlaceholder: 'settings.memoryRemote.token.placeholder',
    tokenShow: 'settings.memoryRemote.token.show',
    tokenHide: 'settings.memoryRemote.token.hide',
    test: 'settings.memoryRemote.test',
    save: 'settings.memoryRemote.save',
    saved: 'settings.memoryRemote.saved',
    problem: 'settings.memoryRemote.problem',
  },
};
