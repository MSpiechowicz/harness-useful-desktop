import { t, type MessageKey } from '$lib/i18n/i18n.svelte';
import type { McpRefusal } from '$lib/ipc/bindings/McpRefusal';
import type { SyncBlockReason } from '$lib/ipc/bindings/SyncBlockReason';
import type { SyncExcludedReason } from '$lib/ipc/bindings/SyncExcludedReason';
import type { SyncInvalidReason } from '$lib/ipc/bindings/SyncInvalidReason';
import type { SyncItemKind } from '$lib/ipc/bindings/SyncItemKind';
import type { SyncItemStatus } from '$lib/ipc/bindings/SyncItemStatus';
import type { SyncMcpPreview } from '$lib/ipc/bindings/SyncMcpPreview';
import type { SyncNote } from '$lib/ipc/bindings/SyncNote';
import type { SyncOutcome } from '$lib/ipc/bindings/SyncOutcome';
import type { SyncSkipReason } from '$lib/ipc/bindings/SyncSkipReason';

export type Tone = 'neutral' | 'accent' | 'success' | 'warning' | 'danger' | 'info';

/** A state chip: the tone says the state, a spinner shows while the item is being set up. */
export interface Chip {
  tone: Tone;
  label: string;
  busy?: boolean;
}

export const KIND_KEYS: Record<SyncItemKind, MessageKey> = {
  marketplace: 'sync.kind.marketplace',
  plugin: 'sync.kind.plugin',
  package: 'sync.kind.package',
  mcpServer: 'sync.kind.mcpServer',
};

const NOTE_KEYS: Record<SyncNote, MessageKey> = {
  envValuesNotShared: 'sync.note.envValuesNotShared',
  headerValuesNotShared: 'sync.note.headerValuesNotShared',
  authNotShared: 'sync.note.authNotShared',
  cwdNotShared: 'sync.note.cwdNotShared',
  filtersNotShared: 'sync.note.filtersNotShared',
  otherFieldsNotShared: 'sync.note.otherFieldsNotShared',
  versionInformational: 'sync.note.versionInformational',
  writtenDisabled: 'sync.note.writtenDisabled',
  needsSharedMarketplace: 'sync.note.needsSharedMarketplace',
};

const EXCLUDED_KEYS: Record<SyncExcludedReason, MessageKey> = {
  containsSecret: 'sync.excluded.containsSecret',
  credentials: 'sync.excluded.credentials',
  localPath: 'sync.excluded.localPath',
  projectScope: 'sync.excluded.projectScope',
  unsupported: 'sync.excluded.unsupported',
  invalid: 'sync.excluded.invalid',
  tooMany: 'sync.excluded.tooMany',
};

const BLOCK_KEYS: Record<SyncBlockReason, MessageKey> = {
  harnessNotInstalled: 'sync.block.harnessNotInstalled',
  marketplaceMissing: 'sync.block.marketplaceMissing',
  notSupported: 'sync.block.notSupported',
};

const INVALID_KEYS: Record<SyncInvalidReason, MessageKey> = {
  malformed: 'sync.invalid.malformed',
  tooLong: 'sync.invalid.tooLong',
  unsafeCharacters: 'sync.invalid.unsafeCharacters',
  unsafeValue: 'sync.invalid.unsafeValue',
  localPath: 'sync.invalid.localPath',
  credentials: 'sync.invalid.credentials',
  containsSecret: 'sync.invalid.containsSecret',
  unsupportedSource: 'sync.invalid.unsupportedSource',
  unsupportedScheme: 'sync.invalid.unsupportedScheme',
  shellCommand: 'sync.invalid.shellCommand',
  badName: 'sync.invalid.badName',
  badKeyName: 'sync.invalid.badKeyName',
  missingField: 'sync.invalid.missingField',
};

const REFUSAL_KEYS: Record<McpRefusal, MessageKey> = {
  unparsable: 'onboarding.setup.refused.unparsable',
  notAnObject: 'onboarding.setup.refused.notAnObject',
  serversNotAnObject: 'onboarding.setup.refused.serversNotAnObject',
  symlink: 'onboarding.setup.refused.symlink',
  notRegularFile: 'onboarding.setup.refused.notRegularFile',
  tooLarge: 'onboarding.setup.refused.tooLarge',
  unreadable: 'onboarding.setup.refused.unreadable',
  writeFailed: 'onboarding.setup.refused.writeFailed',
};

export function noteText(note: SyncNote): string {
  return t(NOTE_KEYS[note]);
}

export function excludedText(reason: SyncExcludedReason): string {
  return t(EXCLUDED_KEYS[reason]);
}

/** The chip of an item's state in a reviewed import. */
export function statusChip(status: SyncItemStatus): Chip {
  switch (status.type) {
    case 'new':
      return { tone: 'accent', label: t('sync.status.new') };
    case 'alreadyPresent':
      return { tone: 'success', label: t('sync.status.alreadyPresent') };
    case 'differentVersion':
      return { tone: 'info', label: t('sync.status.differentVersion') };
    case 'conflict':
      return { tone: 'warning', label: t('sync.status.conflict') };
    case 'blocked':
      return { tone: 'neutral', label: t('sync.status.blocked') };
    case 'invalid':
      return { tone: 'danger', label: t('sync.status.invalid') };
  }
}

/** The sentence that explains a state, or null when the chip says it all. */
export function statusNote(status: SyncItemStatus): string | null {
  switch (status.type) {
    case 'new':
      return null;
    case 'alreadyPresent':
      return null;
    case 'differentVersion':
      return t('sync.statusNote.differentVersion', { installed: status.installed, shared: status.shared });
    case 'conflict':
      return t('sync.statusNote.conflict');
    case 'blocked':
      return t(BLOCK_KEYS[status.reason]);
    case 'invalid':
      return t(INVALID_KEYS[status.reason]);
  }
}

/** The chip of a finished item. */
export function outcomeChip(outcome: SyncOutcome): Chip {
  switch (outcome.type) {
    case 'done':
      return { tone: 'success', label: t('sync.outcome.done') };
    case 'alreadyPresent':
      return { tone: 'success', label: t('sync.outcome.alreadyPresent') };
    case 'conflict':
      return { tone: 'warning', label: t('sync.outcome.conflict') };
    case 'skipped':
      return { tone: 'neutral', label: t('sync.outcome.skipped') };
    case 'failed':
      return { tone: 'danger', label: t('sync.outcome.failed') };
  }
}

function skipText(reason: SyncSkipReason): string {
  switch (reason.type) {
    case 'dependencyFailed':
      return t('sync.skip.dependencyFailed');
    case 'harnessNotInstalled':
      return t('sync.skip.harnessNotInstalled');
    case 'mcpFileRefused':
      return t('sync.skip.mcpFileRefused', { reason: t(REFUSAL_KEYS[reason.refusal]) });
  }
}

/** The sentence under a finished item: why it was skipped, or null when the chip says it all. */
export function outcomeNote(outcome: SyncOutcome): string | null {
  switch (outcome.type) {
    case 'skipped':
      return skipText(outcome.reason);
    case 'conflict':
      return t('sync.statusNote.conflict');
    default:
      return null;
  }
}

/** The names of the variables and headers a server needs, for "Needs …", or null when it needs none. */
export function keysLine(envKeys: string[], headerKeys: string[]): string | null {
  const keys = [...envKeys, ...headerKeys];

  return keys.length > 0 ? t('sync.needsKeys', { keys: keys.join(', ') }) : null;
}

/** The MCP entry an import writes, as it lands under `mcpServers`: exactly the fields the backend writes, arguments on one line. */
export function mcpEntryJson(server: SyncMcpPreview): string {
  const fields: [string, unknown][] = [];

  if (server.transport === 'stdio') {
    fields.push(['command', server.command ?? ''], ['args', server.args]);
  } else {
    fields.push(['type', server.transport], ['url', server.url ?? '']);
  }

  if (!server.enabled) {
    fields.push(['enabled', false]);
  }

  const lines = fields.map(([key, value]) => `  ${JSON.stringify(key)}: ${JSON.stringify(value).replaceAll('","', '", "')}`);

  return `${JSON.stringify(server.name)}: {\n${lines.join(',\n')}\n}`;
}

/** A byte count as "812 B" or "3.4 KB". */
export function sizeText(bytes: number): string {
  return bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KB`;
}

/** The line under the review's title, from whatever of the bundle's origin the backend could safely show. */
export function madeLine(app: string | null, createdAt: string | null): string | null {
  const parts = [app ? t('sync.review.madeWith', { app }) : null, createdAt ? t('sync.review.madeOn', { date: createdAt }) : null].filter((part) => part !== null);

  return parts.length > 0 ? parts.join(' · ') : null;
}

/** A command line as a person would type it: an argument with white space or quotes is single-quoted. */
export function commandLine(argv: string[]): string {
  return argv.map((arg) => (/[\s'"]/.test(arg) ? `'${arg.replaceAll("'", `'\\''`)}'` : arg)).join(' ');
}
