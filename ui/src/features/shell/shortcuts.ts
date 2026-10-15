import { t, type MessageKey } from '$lib/i18n/i18n.svelte';
import { isMacPlatform } from '$lib/utils/shortcuts';

import { VIEWS, viewShortcut } from './views';

export type ShortcutArea = 'Global' | 'Go to' | 'Chat composer' | 'Review & diff' | 'Sessions & lists' | 'Command palette';

/** Order of the areas in the shortcuts overlay. */
export const SHORTCUT_AREAS: readonly ShortcutArea[] = ['Global', 'Go to', 'Chat composer', 'Review & diff', 'Sessions & lists', 'Command palette'];

const AREA_KEYS: Record<ShortcutArea, MessageKey> = {
  Global: 'shell.shortcuts.area.global',
  'Go to': 'shell.shortcuts.area.goTo',
  'Chat composer': 'shell.shortcuts.area.composer',
  'Review & diff': 'shell.shortcuts.area.review',
  'Sessions & lists': 'shell.shortcuts.area.lists',
  'Command palette': 'shell.shortcuts.area.palette',
};

/** The area's name in the language on screen; the area itself stays an id. */
export function areaLabel(area: ShortcutArea): string {
  return t(AREA_KEYS[area]);
}

/** A shortcut the shell itself handles. */
export interface ShortcutAction {
  id: string;
  /** The combo that runs it, e.g. `mod+k`. */
  combo: string;
  /** More combos for the same action (`?` and `mod+/` both open the overlay). */
  also?: string[];
  label: string;
  area: ShortcutArea;
  /**
   * Whether it fires while a text field has focus: `all` combos, only `modified` ones (Ctrl/Cmd or
   * Alt held, so typing is never hijacked), or `none`.
   */
  inInputs: 'all' | 'modified' | 'none';
  run: () => void;
}

/** A shortcut owned by a component further down; listed so the overlay tells the whole truth. */
export interface ShortcutDoc {
  combos: string[];
  /** `and` when the keys belong together (↑ ↓), `or` when each does the job alone. */
  joiner?: 'or' | 'and';
  label: string;
  area: ShortcutArea;
}

/** What the shell can do when a shortcut fires. */
export interface ShellCommands {
  togglePalette: () => void;
  openShortcuts: () => void;
  goBack: () => void;
  goForward: () => void;
  newChat: () => void;
  jumpToChat: () => void;
  /** Shows the next (`1`) or previous (`-1`) chat tab. */
  cycleChat: (step: 1 | -1) => void;
  toggleRail: () => void;
  /** Opens or closes the list drawer of Sessions, Changes, Config or Extensions on a narrow window. */
  togglePane: () => void;
  goTo: (index: number) => void;
}

/**
 * Next and previous chat. Cmd+Tab belongs to the macOS app switcher and never reaches the app, so a Mac uses
 * Cmd+Option+Arrow (plain Cmd+Arrow already moves the caret to the start or end of the composer text).
 */
function chatCycleCombos(mac = isMacPlatform()): { next: string; previous: string } {
  return mac ? { next: 'mod+alt+arrowright', previous: 'mod+alt+arrowleft' } : { next: 'mod+tab', previous: 'mod+shift+tab' };
}

export function globalShortcuts(shell: ShellCommands): ShortcutAction[] {
  const chatCycle = chatCycleCombos();
  const views: ShortcutAction[] = VIEWS.map((view, index) => ({
    id: `go-${view.id}`,
    combo: viewShortcut(view.id),
    also: view.id === 'settings' ? ['mod+,'] : undefined,
    get label() {
      return view.label;
    },
    area: 'Go to',
    inInputs: 'all',
    run: () => shell.goTo(index),
  }));

  return [
    { id: 'palette', combo: 'mod+k', get label() { return t('shell.shortcut.palette'); }, area: 'Global', inInputs: 'all', run: shell.togglePalette },
    { id: 'shortcuts', combo: 'shift+?', also: ['mod+/'], get label() { return t('shell.shortcut.shortcuts'); }, area: 'Global', inInputs: 'modified', run: shell.openShortcuts },
    { id: 'new-chat', combo: 'mod+n', get label() { return t('shell.shortcut.newChat'); }, area: 'Global', inInputs: 'all', run: shell.newChat },
    { id: 'toggle-rail', combo: 'mod+b', get label() { return t('shell.shortcut.toggleRail'); }, area: 'Global', inInputs: 'all', run: shell.toggleRail },
    { id: 'toggle-pane', combo: 'mod+shift+b', get label() { return t('shell.shortcut.togglePane'); }, area: 'Global', inInputs: 'all', run: shell.togglePane },
    { id: 'back', combo: 'alt+arrowleft', get label() { return t('shell.shortcut.back'); }, area: 'Global', inInputs: 'none', run: shell.goBack },
    { id: 'forward', combo: 'alt+arrowright', get label() { return t('shell.shortcut.forward'); }, area: 'Global', inInputs: 'none', run: shell.goForward },
    { id: 'next-chat', combo: chatCycle.next, also: ['mod+pagedown'], get label() { return t('shell.shortcut.nextChat'); }, area: 'Global', inInputs: 'all', run: () => shell.cycleChat(1) },
    { id: 'previous-chat', combo: chatCycle.previous, also: ['mod+pageup'], get label() { return t('shell.shortcut.previousChat'); }, area: 'Global', inInputs: 'all', run: () => shell.cycleChat(-1) },
    { id: 'jump-chat', combo: 'mod+0', get label() { return t('shell.shortcut.jumpChat'); }, area: 'Go to', inInputs: 'all', run: shell.jumpToChat },
    ...views,
  ];
}

export const DOCUMENTED_SHORTCUTS: readonly ShortcutDoc[] = [
  { combos: ['enter'], get label() { return t('shell.shortcut.composer.send'); }, area: 'Chat composer' },
  { combos: ['shift+enter'], get label() { return t('shell.shortcut.composer.newline'); }, area: 'Chat composer' },
  { combos: ['mod+enter'], get label() { return t('shell.shortcut.composer.queue'); }, area: 'Chat composer' },
  { combos: ['arrowup', 'arrowdown'], joiner: 'and', get label() { return t('shell.shortcut.composer.slashMove'); }, area: 'Chat composer' },
  { combos: ['tab'], get label() { return t('shell.shortcut.composer.slashAccept'); }, area: 'Chat composer' },
  { combos: ['escape'], get label() { return t('shell.shortcut.composer.slashDismiss'); }, area: 'Chat composer' },
  { combos: ['mod+end', 'mod+arrowdown'], get label() { return t('shell.shortcut.composer.latest'); }, area: 'Chat composer' },
  { combos: ['alt+arrowup'], get label() { return t('shell.shortcut.composer.takeBack'); }, area: 'Chat composer' },

  { combos: ['arrowup', 'arrowdown'], joiner: 'and', get label() { return t('shell.shortcut.review.move'); }, area: 'Review & diff' },
  { combos: ['mod+enter'], get label() { return t('shell.shortcut.review.save'); }, area: 'Review & diff' },
  { combos: ['escape'], get label() { return t('shell.shortcut.review.cancel'); }, area: 'Review & diff' },
  { combos: ['shift+click'], get label() { return t('shell.shortcut.review.range'); }, area: 'Review & diff' },

  { combos: ['arrowup', 'arrowdown'], joiner: 'and', get label() { return t('shell.shortcut.lists.move'); }, area: 'Sessions & lists' },
  { combos: ['home', 'end'], joiner: 'and', get label() { return t('shell.shortcut.lists.jump'); }, area: 'Sessions & lists' },
  { combos: ['arrowleft', 'arrowright'], joiner: 'and', get label() { return t('shell.shortcut.lists.collapse'); }, area: 'Sessions & lists' },
  { combos: ['shift+arrowright'], get label() { return t('shell.shortcut.lists.resize'); }, area: 'Sessions & lists' },

  { combos: ['arrowup', 'arrowdown'], joiner: 'and', get label() { return t('shell.shortcut.palette.move'); }, area: 'Command palette' },
  { combos: ['enter'], get label() { return t('shell.shortcut.palette.run'); }, area: 'Command palette' },
  { combos: ['shift+enter'], get label() { return t('shell.shortcut.palette.secondary'); }, area: 'Command palette' },
  { combos: ['escape'], get label() { return t('shell.shortcut.palette.close'); }, area: 'Command palette' },
  { combos: ['>'], get label() { return t('shell.shortcut.palette.actions'); }, area: 'Command palette' },
  { combos: ['/'], get label() { return t('shell.shortcut.palette.slash'); }, area: 'Command palette' },
  { combos: ['@'], get label() { return t('shell.shortcut.palette.sessions'); }, area: 'Command palette' },
  { combos: ['#'], get label() { return t('shell.shortcut.palette.files'); }, area: 'Command palette' },
];

export interface ShortcutRow {
  label: string;
  combos: string[];
  /** How the combos relate: alternatives (`or`) or a set of related keys. */
  joiner: 'or' | 'and';
}

export interface ShortcutGroup {
  area: ShortcutArea;
  rows: ShortcutRow[];
}

/** Every shortcut grouped by area for the overlay: the shell's own plus the documented ones. */
export function shortcutGroups(actions: ShortcutAction[]): ShortcutGroup[] {
  return SHORTCUT_AREAS.map((area) => {
    const own = actions.filter((action) => action.area === area).map<ShortcutRow>((action) => ({ label: action.label, combos: [action.combo, ...(action.also ?? [])], joiner: 'or' }));
    const documented = DOCUMENTED_SHORTCUTS.filter((doc) => doc.area === area).map<ShortcutRow>((doc) => ({ label: doc.label, combos: doc.combos, joiner: doc.joiner ?? 'or' }));

    return { area, rows: [...own, ...documented] };
  }).filter((group) => group.rows.length > 0);
}
