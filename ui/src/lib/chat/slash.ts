import type { AgentAction } from '$lib/ipc/bindings/AgentAction';
import type { AgentActionKind } from '$lib/ipc/bindings/AgentActionKind';
import type { CommandSpec } from '$lib/ipc/bindings/CommandSpec';
import type { GuiTarget } from '$lib/ipc/bindings/GuiTarget';
import { fuzzyMatch } from '$lib/utils/fuzzy';

/** What a slash command does once the user picks or sends it. */
export type SlashOutcome =
  /** Text for the composer: the harness runs `/name args` when the user sends it. */
  | { type: 'insert'; text: string }
  | { type: 'send'; text: string }
  | { type: 'action'; kind: AgentActionKind; action: AgentAction }
  /** An RPC that needs a line of text first (`compact` instructions, a session name). */
  | { type: 'ask'; kind: 'compact' | 'name' }
  | { type: 'gui'; target: GuiTarget }
  | { type: 'unsupported'; reason: string };

/** What the chat's harness lets a picked command do. */
export interface PickOptions {
  /** `/compact` takes instructions (`Capabilities.compactInstructions`), so picking it asks for them first. */
  compactInstructions: boolean;
}

const MAX_SUGGESTIONS = 8;

const ASK_FOR_INSTRUCTIONS: PickOptions = { compactInstructions: true };

/** The partial command name while the draft is a lone `/token` (no space yet); otherwise null. */
export function slashQuery(draft: string): string | null {
  const match = /^\/(\S*)$/.exec(draft);

  return match ? match[1] : null;
}

/** Live commands win over the static catalog; commands only the catalog knows are kept. */
export function mergeCatalog(live: CommandSpec[] | null, catalog: CommandSpec[]): CommandSpec[] {
  const known = new Set((live ?? []).map((command) => command.name));

  return [...(live ?? []), ...catalog.filter((command) => !known.has(command.name))];
}

export function filterCommands(commands: CommandSpec[], query: string, limit: number = MAX_SUGGESTIONS): CommandSpec[] {
  const ranked = commands.flatMap((command) => {
    const byName = fuzzyMatch(query, command.name);
    const byDescription = query.length >= 3 ? fuzzyMatch(query, command.description) : null;
    const score = byName ? byName.score + 1000 : (byDescription?.score ?? null);

    return score === null ? [] : [{ command, score }];
  });

  return ranked
    .sort((a, b) => b.score - a.score || a.command.name.localeCompare(b.command.name))
    .slice(0, limit)
    .map((entry) => entry.command);
}

function rpcOutcome(kind: AgentActionKind, args: string, interactive: boolean, options: PickOptions): SlashOutcome {
  switch (kind) {
    case 'compact':
      return args === '' && interactive && options.compactInstructions ? { type: 'ask', kind: 'compact' } : { type: 'action', kind, action: { type: 'compact', instructions: args === '' ? null : args } };

    case 'setSessionName':
      return args === '' ? { type: 'ask', kind: 'name' } : { type: 'action', kind, action: { type: 'setSessionName', name: args } };

    case 'newSession':
      return { type: 'action', kind, action: { type: 'newSession' } };

    case 'clone':
      return { type: 'action', kind, action: { type: 'clone' } };

    case 'exportHtml':
      return { type: 'action', kind, action: { type: 'exportHtml' } };

    case 'getSessionStats':
      return { type: 'action', kind, action: { type: 'getSessionStats' } };

    case 'getLastAssistantText':
      return { type: 'action', kind, action: { type: 'getLastAssistantText' } };

    case 'abort':
      return { type: 'action', kind, action: { type: 'abort' } };
  }
}

function invocation(command: CommandSpec, args: string, interactive: boolean, options: PickOptions = ASK_FOR_INSTRUCTIONS): SlashOutcome {
  const how = command.invocation;

  switch (how.kind) {
    case 'promptText':
      return interactive ? { type: 'insert', text: `/${command.name} ` } : { type: 'send', text: args === '' ? `/${command.name}` : `/${command.name} ${args}` };

    case 'rpc':
      return rpcOutcome(how.action, args, interactive, options);

    case 'gui':
      return { type: 'gui', target: how.target };

    case 'unsupported':
      return { type: 'unsupported', reason: how.reason };
  }
}

/** Picking a command from the suggestion list. */
export function acceptCommand(command: CommandSpec, options: PickOptions = ASK_FOR_INSTRUCTIONS): SlashOutcome {
  return invocation(command, '', true, options);
}

/**
 * Sending a draft that starts with `/`. A known command runs the way its catalog entry says;
 * anything else goes to the harness as typed (it may know commands the catalog does not).
 */
export function resolveSlash(text: string, catalog: CommandSpec[]): SlashOutcome {
  const trimmed = text.trim();
  const match = /^\/(\S+)(?:\s+([\s\S]*))?$/.exec(trimmed);
  const command = match ? catalog.find((candidate) => candidate.name === match[1]) : undefined;

  if (!match || !command) {
    return { type: 'send', text };
  }

  return invocation(command, (match[2] ?? '').trim(), false);
}
