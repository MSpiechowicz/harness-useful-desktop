import Blocks from '@lucide/svelte/icons/blocks';
import GitBranch from '@lucide/svelte/icons/git-branch';
import GitCompare from '@lucide/svelte/icons/git-compare';
import MessageSquare from '@lucide/svelte/icons/message-square';
import Orbit from '@lucide/svelte/icons/orbit';
import Settings from '@lucide/svelte/icons/settings';
import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';

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

/** Rail order; position + 1 is the Ctrl/Cmd digit shortcut. */
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

/** Ctrl/Cmd digit shortcut for a view, e.g. `mod+3`. */
export function viewShortcut(id: ViewId): string {
  return `mod+${VIEWS.findIndex((view) => view.id === id) + 1}`;
}
