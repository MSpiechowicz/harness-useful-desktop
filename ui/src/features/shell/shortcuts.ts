import { VIEWS, viewShortcut } from './views';

export type ShortcutArea = 'Global' | 'Go to' | 'Chat composer' | 'Review & diff' | 'Sessions & lists' | 'Command palette';

/** Order of the areas in the shortcuts overlay. */
export const SHORTCUT_AREAS: readonly ShortcutArea[] = ['Global', 'Go to', 'Chat composer', 'Review & diff', 'Sessions & lists', 'Command palette'];

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
  goTo: (index: number) => void;
}

export function globalShortcuts(shell: ShellCommands): ShortcutAction[] {
  const views: ShortcutAction[] = VIEWS.map((view, index) => ({
    id: `go-${view.id}`,
    combo: viewShortcut(view.id),
    also: view.id === 'settings' ? ['mod+,'] : undefined,
    label: view.label,
    area: 'Go to',
    inInputs: 'all',
    run: () => shell.goTo(index),
  }));

  return [
    { id: 'palette', combo: 'mod+k', label: 'Open the command palette', area: 'Global', inInputs: 'all', run: shell.togglePalette },
    { id: 'shortcuts', combo: 'shift+?', also: ['mod+/'], label: 'Show keyboard shortcuts', area: 'Global', inInputs: 'modified', run: shell.openShortcuts },
    { id: 'new-chat', combo: 'mod+n', label: 'New chat', area: 'Global', inInputs: 'all', run: shell.newChat },
    { id: 'toggle-rail', combo: 'mod+b', label: 'Expand or collapse the sidebar', area: 'Global', inInputs: 'all', run: shell.toggleRail },
    { id: 'back', combo: 'alt+arrowleft', label: 'Back', area: 'Global', inInputs: 'none', run: shell.goBack },
    { id: 'forward', combo: 'alt+arrowright', label: 'Forward', area: 'Global', inInputs: 'none', run: shell.goForward },
    { id: 'next-chat', combo: 'mod+tab', also: ['mod+pagedown'], label: 'Next chat', area: 'Global', inInputs: 'all', run: () => shell.cycleChat(1) },
    { id: 'previous-chat', combo: 'mod+shift+tab', also: ['mod+pageup'], label: 'Previous chat', area: 'Global', inInputs: 'all', run: () => shell.cycleChat(-1) },
    { id: 'jump-chat', combo: 'mod+0', label: 'Jump to the active chat', area: 'Go to', inInputs: 'all', run: shell.jumpToChat },
    ...views,
  ];
}

export const DOCUMENTED_SHORTCUTS: readonly ShortcutDoc[] = [
  { combos: ['enter'], label: 'Send the message (steer the agent while it works)', area: 'Chat composer' },
  { combos: ['shift+enter'], label: 'New line', area: 'Chat composer' },
  { combos: ['mod+enter'], label: 'Queue a follow-up while the agent works', area: 'Chat composer' },
  { combos: ['arrowup', 'arrowdown'], joiner: 'and', label: 'Move through slash commands after typing /', area: 'Chat composer' },
  { combos: ['tab'], label: 'Accept the highlighted slash command', area: 'Chat composer' },
  { combos: ['escape'], label: 'Dismiss the slash menu', area: 'Chat composer' },

  { combos: ['arrowup', 'arrowdown'], joiner: 'and', label: 'Move between changed files', area: 'Review & diff' },
  { combos: ['mod+enter'], label: 'Save a line comment', area: 'Review & diff' },
  { combos: ['escape'], label: 'Cancel a line comment', area: 'Review & diff' },
  { combos: ['shift+click'], label: 'Extend a comment over a range of lines (click a line number)', area: 'Review & diff' },

  { combos: ['arrowup', 'arrowdown'], joiner: 'and', label: 'Move through sessions, tree entries and files', area: 'Sessions & lists' },
  { combos: ['home', 'end'], joiner: 'and', label: 'Jump to the first or last entry', area: 'Sessions & lists' },
  { combos: ['arrowleft', 'arrowright'], joiner: 'and', label: 'Collapse or expand a configuration key; resize a focused divider', area: 'Sessions & lists' },
  { combos: ['shift+arrowright'], label: 'Resize a focused divider in larger steps', area: 'Sessions & lists' },

  { combos: ['arrowup', 'arrowdown'], joiner: 'and', label: 'Move through results', area: 'Command palette' },
  { combos: ['enter'], label: 'Run the highlighted result', area: 'Command palette' },
  { combos: ['shift+enter'], label: 'Run its secondary action, such as Resume in chat', area: 'Command palette' },
  { combos: ['escape'], label: 'Close', area: 'Command palette' },
  { combos: ['>'], label: 'Only actions and views', area: 'Command palette' },
  { combos: ['/'], label: 'Slash commands of the active chat', area: 'Command palette' },
  { combos: ['@'], label: 'Sessions of this project', area: 'Command palette' },
  { combos: ['#'], label: 'Changed files', area: 'Command palette' },
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
