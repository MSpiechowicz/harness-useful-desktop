import FileText from '@lucide/svelte/icons/file-text';
import FilePlus from '@lucide/svelte/icons/file-plus';
import FolderSearch from '@lucide/svelte/icons/folder-search';
import Globe from '@lucide/svelte/icons/globe';
import Bot from '@lucide/svelte/icons/bot';
import Pencil from '@lucide/svelte/icons/pencil';
import Search from '@lucide/svelte/icons/search';
import Terminal from '@lucide/svelte/icons/terminal';
import Wrench from '@lucide/svelte/icons/wrench';

import type { IconComponent } from '$lib/components/icon';
import type { JsonValue } from '$lib/ipc/bindings/serde_json/JsonValue';

import { isJsonObject } from './tools';
import type { ToolCallState } from './types';

export type ToolKind = 'read' | 'edit' | 'write' | 'bash' | 'grep' | 'glob' | 'web' | 'task' | 'other';

const KINDS: Record<ToolKind, readonly string[]> = {
  read: ['read', 'view', 'cat', 'open'],
  edit: ['edit', 'ast_edit', 'multi_edit', 'notebook_edit', 'apply_patch', 'patch'],
  write: ['write', 'create', 'write_file'],
  bash: ['bash', 'shell', 'exec', 'run', 'terminal'],
  grep: ['grep', 'ast_grep', 'search', 'rg'],
  glob: ['glob', 'find', 'ls', 'list', 'tree'],
  web: ['web_search', 'web_fetch', 'webfetch', 'fetch', 'browser', 'read_url', 'websearch'],
  task: ['task', 'subagent', 'agent', 'delegate'],
  other: [],
};

export const TOOL_ICONS: Record<ToolKind, IconComponent> = {
  read: FileText,
  edit: Pencil,
  write: FilePlus,
  bash: Terminal,
  grep: Search,
  glob: FolderSearch,
  web: Globe,
  task: Bot,
  other: Wrench,
};

export function toolKind(name: string): ToolKind {
  const lowered = name.toLowerCase();
  const found = (Object.keys(KINDS) as ToolKind[]).find((kind) => KINDS[kind].includes(lowered));

  return found ?? 'other';
}

/** Tools that change files; their cards list the touched paths. */
export function changesFiles(kind: ToolKind): boolean {
  return kind === 'edit' || kind === 'write';
}

const PRIMARY_ARGS = ['command', 'path', 'file_path', 'pattern', 'query', 'url', 'description', 'prompt'] as const;
const PRIMARY_MAX = 90;

/** The one argument worth showing next to the title: the command, path, pattern or URL. */
export function primaryArg(args: JsonValue | null): string | null {
  if (!isJsonObject(args)) {
    return null;
  }

  for (const name of PRIMARY_ARGS) {
    const value = args[name];
    if (typeof value === 'string' && value.trim() !== '') {
      const single = value.replace(/\s+/g, ' ').trim();

      return single.length > PRIMARY_MAX ? `${single.slice(0, PRIMARY_MAX - 1)}…` : single;
    }
  }

  return null;
}

/** The shell command a bash-like call runs, when it has one. */
export function commandOf(call: ToolCallState): string | null {
  if (toolKind(call.name) !== 'bash' || !isJsonObject(call.args)) {
    return null;
  }

  const command = call.args.command;

  return typeof command === 'string' ? command : null;
}

/**
 * A tool name for people: `mcp__harness_memory_note_search` → "Harness memory note search",
 * `mcp__server__tool_name` → "Server · Tool name", `web_search` → "Web search".
 */
export function humanToolName(name: string): string {
  const words = (part: string) => {
    const spaced = part.replace(/[_-]+/g, ' ').trim();

    return spaced.charAt(0).toUpperCase() + spaced.slice(1);
  };

  const parts = name.split('__').filter((part) => part !== '' && part !== 'mcp');
  const shown = parts.map(words).filter((part) => part !== '');

  return shown.length > 0 ? shown.join(' · ') : name;
}

/** Card title: the agent's stated intent, else the readable tool name. */
export function toolTitle(call: ToolCallState): string {
  return call.intent?.trim() || humanToolName(call.name);
}
