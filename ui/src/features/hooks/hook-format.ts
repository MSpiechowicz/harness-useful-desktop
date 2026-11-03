import CornerDownRight from '@lucide/svelte/icons/corner-down-right';
import FilePlus from '@lucide/svelte/icons/file-plus';
import Flag from '@lucide/svelte/icons/flag';
import LogOut from '@lucide/svelte/icons/log-out';
import Play from '@lucide/svelte/icons/play';
import Send from '@lucide/svelte/icons/send';
import SquareTerminal from '@lucide/svelte/icons/square-terminal';
import Wrench from '@lucide/svelte/icons/wrench';

import { MAX_PER_SCOPE, type SortValue, sortDocs } from '$lib/components/docs/doc-groups';
import type { ScopedDoc } from '$lib/components/docs/doc-scope';
import type { IconComponent } from '$lib/components/icon';
import { t } from '$lib/i18n/i18n.svelte';
import type { MessageKey } from '$lib/i18n/locales/en';
import type { AppPrefs } from '$lib/ipc/bindings/AppPrefs';
import type { Hook } from '$lib/ipc/bindings/Hook';
import type { HookAction } from '$lib/ipc/bindings/HookAction';
import type { HookEvent } from '$lib/ipc/bindings/HookEvent';
import type { HookListing } from '$lib/ipc/bindings/HookListing';
import type { HookStatus } from '$lib/ipc/bindings/HookStatus';
import type { HookRequirement } from '$lib/ipc/bindings/HookRequirement';
import type { HookView } from '$lib/ipc/bindings/HookView';
import type { Sort } from '$lib/components/table';
import type { HookSource, HookStatusFilter, HookWhen } from '$lib/stores/view-state.svelte';

import { spelledOut } from '../memory/script-format';
import type { Companion } from '../onboarding/setup-model';
import { switchedOn } from './hook-team';

export { MAX_PER_SCOPE };

/** The events in the order they happen in a chat. */
export const EVENTS: readonly HookEvent[] = ['sessionStart', 'promptSubmit', 'toolEnd', 'turnEnd', 'sessionEnd'];

export type ActionKind = HookAction['type'];

/** What a hook can do at each event, by the format's table; anything else is an invalid file. */
export const VALID_ACTIONS: Record<HookEvent, readonly ActionKind[]> = {
  sessionStart: ['addContext', 'command'],
  promptSubmit: ['addContext', 'command'],
  toolEnd: ['followUp', 'command'],
  turnEnd: ['followUp', 'command'],
  sessionEnd: ['command'],
};

export const EVENT_LABELS: Record<HookEvent, MessageKey> = {
  sessionStart: 'hooks.event.sessionStart',
  promptSubmit: 'hooks.event.promptSubmit',
  toolEnd: 'hooks.event.toolEnd',
  turnEnd: 'hooks.event.turnEnd',
  sessionEnd: 'hooks.event.sessionEnd',
};

/** One line on when the event happens: "When a chat begins". */
export const EVENT_HINTS: Record<HookEvent, MessageKey> = {
  sessionStart: 'hooks.event.sessionStart.hint',
  promptSubmit: 'hooks.event.promptSubmit.hint',
  toolEnd: 'hooks.event.toolEnd.hint',
  turnEnd: 'hooks.event.turnEnd.hint',
  sessionEnd: 'hooks.event.sessionEnd.hint',
};

/** The same moment as a clause that follows "Runs": "when a chat begins". */
export const EVENT_WHEN: Record<HookEvent, MessageKey> = {
  sessionStart: 'hooks.event.sessionStart.when',
  promptSubmit: 'hooks.event.promptSubmit.when',
  toolEnd: 'hooks.event.toolEnd.when',
  turnEnd: 'hooks.event.turnEnd.when',
  sessionEnd: 'hooks.event.sessionEnd.when',
};

export const EVENT_ICONS: Record<HookEvent, IconComponent> = {
  sessionStart: Play,
  promptSubmit: Send,
  toolEnd: Wrench,
  turnEnd: Flag,
  sessionEnd: LogOut,
};

export const ACTION_LABELS: Record<ActionKind, MessageKey> = {
  addContext: 'hooks.action.addContext',
  followUp: 'hooks.action.followUp',
  command: 'hooks.action.command',
};

export const ACTION_ICONS: Record<ActionKind, IconComponent> = {
  addContext: FilePlus,
  followUp: CornerDownRight,
  command: SquareTerminal,
};

export const STATUS_LABELS: Record<HookStatus, MessageKey> = {
  active: 'hooks.status.active',
  off: 'hooks.status.off',
  pending: 'hooks.status.pending',
  paused: 'hooks.status.paused',
  invalid: 'hooks.status.invalid',
  duplicate: 'hooks.status.duplicate',
};

export const STATUS_TONES = { active: 'success', off: 'neutral', pending: 'warning', paused: 'info', invalid: 'danger', duplicate: 'neutral' } as const satisfies Record<HookStatus, string>;

/** The statuses in the order the Status filter lists them. */
export const STATUSES: readonly HookStatus[] = ['active', 'pending', 'off', 'paused', 'invalid', 'duplicate'];

/** The key a hook is listed, opened and worked on by: a team hook can share its id with a local one, so the team's carries a prefix. */
export function viewKey(view: HookView): string {
  return view.origin === 'team' ? `team:${view.id}` : view.id;
}

/** The name as the file has it: an invalid file may have none, then its id stands in. */
function rawTitle(view: HookView): string {
  return view.hook?.name ?? view.name ?? view.id;
}

/** The name a hook is listed under. A file an agent wrote may hide characters in it, so each is spelled out. */
export function hookTitle(view: HookView): string {
  return spelledOut(rawTitle(view));
}

/** The text a hook sends or the command it runs. */
export function actionText(action: HookAction): string {
  return action.type === 'command' ? action.command : action.text;
}

/** The hooks the Hooks view and its rail badge count as waiting for the user. */
export function pendingCount(listing: HookListing | null): number {
  return listing?.hooks.filter((view) => view.status === 'pending').length ?? 0;
}

export function inWhen(view: HookView, when: HookWhen): boolean {
  return when === 'all' || view.hook?.event === when;
}

export function inStatus(view: HookView, status: HookStatusFilter): boolean {
  return status === 'all' || view.status === status;
}

export function inSource(view: HookView, source: HookSource): boolean {
  return source === 'all' || (source === 'team') === (view.origin === 'team');
}

/** Whether a hook answers the search: its name, description, text or command, and tool filter. */
export function matchesQuery(view: HookView, query: string): boolean {
  const needle = query.trim().toLowerCase();

  if (needle === '') {
    return true;
  }

  const hook = view.hook;
  const haystack = [rawTitle(view), hook?.description, hook?.matcher, hook ? actionText(hook.action) : view.problem];

  return haystack.some((part) => part?.toLowerCase().includes(needle));
}

/** How many hooks each choice of the two filters holds (of all the hooks, not of the search or the other filter). */
export interface FilterCounts {
  when: Record<HookWhen, number>;
  status: Record<HookStatusFilter, number>;
  source: Record<HookSource, number>;
}

export function filterCounts(views: HookView[]): FilterCounts {
  const when = { all: views.length, sessionStart: 0, promptSubmit: 0, toolEnd: 0, turnEnd: 0, sessionEnd: 0 };
  const status = { all: views.length, active: 0, pending: 0, off: 0, paused: 0, invalid: 0, duplicate: 0 };
  const source = { all: views.length, mine: 0, team: 0 };

  for (const view of views) {
    status[view.status] += 1;
    source[view.origin === 'team' ? 'team' : 'mine'] += 1;

    if (view.hook) {
      when[view.hook.event] += 1;
    }
  }

  return { when, status, source };
}

/** Where a hook applies, as the Scope column and the Scope filter read it: the file's own scope (none for a file that is not a hook) and the project it names. */
export function hookDoc(view: HookView): ScopedDoc {
  return { origin: view.origin, scope: view.hook?.scope ?? null, root: view.root, repoKey: view.repoKey };
}

/** What each sortable column of the list sorts a hook by: the name, the event in the order they happen in a chat, what it does and its status as the reader sees them, and whether it is switched on. */
const SORT_VALUES: Record<string, (view: HookView) => SortValue> = {
  hook: (view) => hookTitle(view),
  when: (view) => (view.hook ? EVENTS.indexOf(view.hook.event) : null),
  does: (view) => (view.hook ? t(ACTION_LABELS[view.hook.action.type]) : null),
  status: (view) => t(STATUS_LABELS[view.status]),
  on: (view) => Number(switchedOn(view)),
};

/** How many hooks "Delete all" removes: the global ones, the files that are not hooks and those of the open project (by its root). Another project's hooks and the team's stay. */
export function deletableCount(views: HookView[], openRoot: string | null): number {
  return views.filter((view) => view.origin === 'local' && (view.hook === null || view.hook.scope === 'global' || (openRoot !== null && view.hook.project === openRoot))).length;
}

/** The hooks in the order of a column; null keeps the run order (see `sortDocs`). `scopeValue` is what the Scope column sorts a hook by. */
export function sortHooks(views: HookView[], sort: Sort | null, scopeValue: (view: HookView) => SortValue): HookView[] {
  return sortDocs(views, sort, { ...SORT_VALUES, scope: scopeValue });
}

/** The tool names of a `toolEnd` filter, one chip each. */
export function matcherNames(matcher: string | null | undefined): string[] {
  return matcher ? matcher.split('|').filter((name) => name !== '') : [];
}

/** The number of Unicode code points, which is how the format counts characters. */
export function length(text: string): number {
  return [...text].length;
}

/** What a hook can need: a companion with its own installer, or Harness Skills, which are installed per harness from the Ecosystem page. */
export type HookMember = Companion | 'skills';

/** What an installer button tells its parent. */
export interface InstallEvents {
  /** Runs once the installer finished well. */
  onfinished?: () => void;
}

/** The Harness family member each requirement stands for. */
const MEMBER_OF: Record<HookRequirement, HookMember> = { 'harness-memory': 'memory', 'harness-dashboard': 'dashboard', 'harness-knowledge': 'knowledge', 'harness-code': 'code', 'useful-skills': 'skills' };

export function memberOf(requirement: HookRequirement): HookMember {
  return MEMBER_OF[requirement];
}

export const MEMBER_LABELS: Record<HookMember, MessageKey> = { memory: 'hooks.member.memory', dashboard: 'hooks.member.dashboard', knowledge: 'hooks.member.knowledge', code: 'hooks.member.code', skills: 'hooks.member.skills' };

/** How often a Harness family hook runs, by the event it fires on. An event the catalog does not use is named as its event. */
export const CADENCE_LABELS: Partial<Record<HookEvent, MessageKey>> = { sessionStart: 'hooks.family.cadence.once', promptSubmit: 'hooks.family.cadence.every', turnEnd: 'hooks.family.cadence.turnEnd' };

export function cadenceLabel(event: HookEvent): MessageKey {
  return CADENCE_LABELS[event] ?? EVENT_LABELS[event];
}

/** The Harness family member a paused hook waits for, and whether it is installed but switched off (Harness Memory, Harness Knowledge and Harness Code have such a switch). */
export interface WaitingFor {
  member: HookMember;
  off: boolean;
}

/** Which of the members with a switch are switched on. */
export interface MemberSwitches {
  memory: boolean;
  knowledge: boolean;
  code: boolean;
}

/** The switches as the preferences hold them (a switch not read yet counts as on), and the code index, which the one Code graph switch turns on and off with Harness Code's registration. */
export function memberSwitches(value: Pick<AppPrefs, 'memoryEnabled' | 'knowledgeEnabled' | 'codeEnabled'> | null, codeIndexOn: boolean): MemberSwitches {
  return { memory: value?.memoryEnabled !== false, knowledge: value?.knowledgeEnabled !== false, code: value?.codeEnabled !== false && codeIndexOn };
}

/** What a paused hook waits for, or null when it needs nothing. */
export function waitingFor(hook: Hook, switches: MemberSwitches): WaitingFor | null {
  const requirement = hook.requires[0];

  if (requirement === undefined) {
    return null;
  }

  const member = memberOf(requirement);

  return { member, off: (member === 'memory' && !switches.memory) || (member === 'knowledge' && !switches.knowledge) || (member === 'code' && !switches.code) };
}

/** The Settings section whose switch turns `member` on again. Harness Skills have no switch (they are never "off"), so they point at the memory section like the dashboard does. */
export function switchSection(member: HookMember): 'memory' | 'knowledge' | 'code' {
  switch (member) {
    case 'knowledge':
    case 'code':
      return member;
    case 'memory':
    case 'dashboard':
    case 'skills':
      return 'memory';
  }
}
