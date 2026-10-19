import Blocks from '@lucide/svelte/icons/blocks';
import CalendarClock from '@lucide/svelte/icons/calendar-clock';
import FileCode from '@lucide/svelte/icons/file-code';
import GitBranch from '@lucide/svelte/icons/git-branch';
import GitCompare from '@lucide/svelte/icons/git-compare';
import MessageSquare from '@lucide/svelte/icons/message-square';
import NotebookText from '@lucide/svelte/icons/notebook-text';
import Orbit from '@lucide/svelte/icons/orbit';
import Settings from '@lucide/svelte/icons/settings';
import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';
import Webhook from '@lucide/svelte/icons/webhook';

import { t } from '$lib/i18n/i18n.svelte';
import type { IconComponent } from '$lib/components/icon';
import type { ViewId } from '$lib/stores/nav.svelte';

export interface ViewMeta {
  id: ViewId;
  label: string;
  icon: IconComponent;
  /** Rail group heading shown above this view, which starts a new group. */
  section?: string;
  /** Empty-state copy for views whose feature has not shipped yet. */
  heading: string;
  body: string;
}

/** Rail order; position + 1 is the Ctrl/Cmd digit shortcut for the first nine views. */
export const VIEWS: readonly ViewMeta[] = [
  {
    id: 'chat',
    get label() {
      return t('shell.view.chat');
    },
    icon: MessageSquare,
    get heading() {
      return t('shell.view.chat.heading');
    },
    get body() {
      return t('shell.view.chat.body');
    },
  },
  {
    id: 'sessions',
    get label() {
      return t('shell.view.sessions');
    },
    icon: GitBranch,
    get heading() {
      return t('shell.view.sessions.heading');
    },
    get body() {
      return t('shell.view.sessions.body');
    },
  },
  {
    id: 'changes',
    get label() {
      return t('shell.view.changes');
    },
    icon: GitCompare,
    get heading() {
      return t('shell.view.changes.heading');
    },
    get body() {
      return t('shell.view.changes.body');
    },
  },
  {
    id: 'config',
    get label() {
      return t('shell.view.config');
    },
    get section() {
      return t('shell.section.harness');
    },
    icon: SlidersHorizontal,
    get heading() {
      return t('shell.view.config.heading');
    },
    get body() {
      return t('shell.view.config.body');
    },
  },
  {
    id: 'extensions',
    get label() {
      return t('shell.view.extensions');
    },
    icon: Blocks,
    get heading() {
      return t('shell.view.extensions.heading');
    },
    get body() {
      return t('shell.view.extensions.body');
    },
  },
  {
    id: 'ecosystem',
    get label() {
      return t('shell.view.ecosystem');
    },
    icon: Orbit,
    get heading() {
      return t('shell.view.ecosystem.heading');
    },
    get body() {
      return t('shell.view.ecosystem.body');
    },
  },
  {
    id: 'notes',
    get label() {
      return t('shell.view.notes');
    },
    icon: NotebookText,
    get heading() {
      return t('shell.view.notes.heading');
    },
    get body() {
      return t('shell.view.notes.body');
    },
  },
  {
    id: 'scripts',
    get label() {
      return t('shell.view.scripts');
    },
    icon: FileCode,
    get heading() {
      return t('shell.view.scripts.heading');
    },
    get body() {
      return t('shell.view.scripts.body');
    },
  },
  {
    id: 'hooks',
    get label() {
      return t('shell.view.hooks');
    },
    icon: Webhook,
    get heading() {
      return t('shell.view.hooks.heading');
    },
    get body() {
      return t('shell.view.hooks.body');
    },
  },
  {
    id: 'routines',
    get label() {
      return t('shell.view.routines');
    },
    icon: CalendarClock,
    get heading() {
      return t('shell.view.routines.heading');
    },
    get body() {
      return t('shell.view.routines.body');
    },
  },
  {
    id: 'settings',
    get label() {
      return t('shell.view.settings');
    },
    get section() {
      return t('shell.section.app');
    },
    icon: Settings,
    get heading() {
      return t('shell.view.settings.heading');
    },
    get body() {
      return t('shell.view.settings.body');
    },
  },
];

export function viewMeta(id: ViewId): ViewMeta {
  return VIEWS.find((view) => view.id === id) ?? VIEWS[0];
}

/** The most views that have a Ctrl/Cmd digit: 1 to 9. */
const DIGIT_SHORTCUTS = 9;

/** Settings keeps only this one: the views past the ninth have no digit, and the preferences are where `,` has always been. */
export const SETTINGS_SHORTCUT = 'mod+,';

/** The combo that opens a view: Settings has Ctrl/Cmd+comma, the first nine views their digit (`mod+3`), and one past the ninth none. */
export function viewShortcut(id: ViewId): string | null {
  if (id === 'settings') {
    return SETTINGS_SHORTCUT;
  }

  const index = VIEWS.findIndex((view) => view.id === id);

  return index < DIGIT_SHORTCUTS ? `mod+${index + 1}` : null;
}

/** The shortcut the rail and the palette show for a view; null when it has none, and then nothing is shown. */
export function viewHint(id: ViewId): string | null {
  return viewShortcut(id);
}
