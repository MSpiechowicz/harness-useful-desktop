import FileText from '@lucide/svelte/icons/file-text';
import FilePlus from '@lucide/svelte/icons/file-plus';
import FolderSearch from '@lucide/svelte/icons/folder-search';
import Globe from '@lucide/svelte/icons/globe';
import Bot from '@lucide/svelte/icons/bot';
import Pencil from '@lucide/svelte/icons/pencil';
import Search from '@lucide/svelte/icons/search';
import Terminal from '@lucide/svelte/icons/terminal';
import Wrench from '@lucide/svelte/icons/wrench';

import { FAMILY_NAMES, type FamilyProduct } from '$lib/brand/family';
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

/** The one argument worth showing next to the title (the command, path, pattern or URL), shortened to fit a card line. */
export function primaryArg(args: JsonValue | null): string | null {
  const single = fullPrimaryArg(args);

  return single !== null && single.length > PRIMARY_MAX ? `${single.slice(0, PRIMARY_MAX - 1)}…` : single;
}

/** The same argument in full on one line, for where nothing may be cut off (an approval). */
export function fullPrimaryArg(args: JsonValue | null): string | null {
  if (!isJsonObject(args)) {
    return null;
  }

  for (const name of PRIMARY_ARGS) {
    const value = args[name];
    if (typeof value === 'string' && value.trim() !== '') {
      return value.replace(/\s+/g, ' ').trim();
    }
  }

  return null;
}

/** The skill a call loads as the harness names it: Claude Code's `Skill` says it in `skill` with its plugin in front, OpenCode's `skill` in `name`. */
export function skillOf(call: ToolCallState): string | null {
  if (!isJsonObject(call.args)) {
    return null;
  }

  const named = call.name === 'Skill' ? call.args.skill : call.name === 'skill' ? call.args.name : null;

  return typeof named === 'string' && named.trim() !== '' ? named.trim() : null;
}

/** The skill and the arguments Claude Code passes it, in full, for an approval. */
export function skillApproval(call: ToolCallState): string | null {
  const skill = skillOf(call);
  const args = isJsonObject(call.args) ? call.args.args : null;

  return skill !== null && typeof args === 'string' && args.trim() !== '' ? `${skill} ${args.trim()}` : skill;
}

/** The same skill for a card line: without its plugin, or the whole name when nothing follows the plugin. */
export function shortSkillOf(call: ToolCallState): string | null {
  const skill = skillOf(call);

  return skill?.split(':').pop() || skill;
}

/** Arguments that add nothing to what a shell command does. */
const COMMAND_NOISE = new Set(['command', 'description', 'timeout']);

/**
 * What an approval shows beside a call's command, or instead of one: what else the command is given (where it runs),
 * the path of a file tool, and every argument of any other tool, so none is hidden behind its main one.
 */
export function approvalArgument(call: ToolCallState): string | null {
  const { args } = call;
  if (!isJsonObject(args) || Object.keys(args).length === 0) {
    return null;
  }

  if (commandOf(call) !== null) {
    const rest = Object.entries(args).filter(([name]) => !COMMAND_NOISE.has(name));

    return rest.length > 0 ? JSON.stringify(Object.fromEntries(rest)) : null;
  }

  const kind = toolKind(call.name);
  const single = fullPrimaryArg(args);
  if (single !== null && (kind === 'edit' || kind === 'write' || kind === 'read' || Object.keys(args).length === 1)) {
    return single;
  }

  return JSON.stringify(args);
}

/** The shell command a bash-like call runs, when it has one. */
export function commandOf(call: ToolCallState): string | null {
  if (toolKind(call.name) !== 'bash' || !isJsonObject(call.args)) {
    return null;
  }

  const command = call.args.command;

  return typeof command === 'string' ? command : null;
}

/** A family member's tool under any harness's naming, the same forms as `harnessToolName`: the member, then the tool. */
const FAMILY_TOOL = new RegExp(`^(?:mcp__)?(?:plugin_[A-Za-z0-9-]+_)?harness[-_](${Object.keys(FAMILY_NAMES).join('|')})(?:__|_)([a-z][a-z0-9_]*)$`);

/**
 * A tool name for people: `mcp__server__tool_name` → "Server · Tool name", `web_search` → "Web search". A family member keeps its
 * product name whatever the harness calls the tool: `harness-code_code_map` and `mcp__harness-code__code_map` → "Harness Code · Code map".
 */
export function humanToolName(name: string): string {
  const words = (part: string) => {
    const spaced = part.replace(/[_-]+/g, ' ').trim();

    return spaced.charAt(0).toUpperCase() + spaced.slice(1);
  };

  const family = FAMILY_TOOL.exec(name);
  if (family) {
    return `${FAMILY_NAMES[family[1] as FamilyProduct]} · ${words(family[2])}`;
  }

  const parts = name.split('__').filter((part) => part !== '' && part !== 'mcp');
  const shown = parts.map(words).filter((part) => part !== '');

  return shown.length > 0 ? shown.join(' · ') : name;
}

/** Card title: the agent's stated intent, else the readable tool name. */
export function toolTitle(call: ToolCallState): string {
  return call.intent?.trim() || humanToolName(call.name);
}
