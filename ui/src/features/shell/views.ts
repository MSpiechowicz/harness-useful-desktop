import Blocks from '@lucide/svelte/icons/blocks';
import GitBranch from '@lucide/svelte/icons/git-branch';
import GitCompare from '@lucide/svelte/icons/git-compare';
import MessageSquare from '@lucide/svelte/icons/message-square';
import Orbit from '@lucide/svelte/icons/orbit';
import Settings from '@lucide/svelte/icons/settings';
import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';

import type { IconComponent } from '$lib/components/icon';
import type { ViewId } from '$lib/stores/nav.svelte';

export interface ViewMeta {
  id: ViewId;
  label: string;
  icon: IconComponent;
  /** Empty-state copy for views whose feature has not shipped yet. */
  heading: string;
  body: string;
}

/** Rail order; position + 1 is the Ctrl/Cmd digit shortcut. */
export const VIEWS: readonly ViewMeta[] = [
  {
    id: 'chat',
    label: 'Chat',
    icon: MessageSquare,
    heading: 'Talk to your agent',
    body: 'Stream responses, follow tool calls and steer your harness without leaving the window.',
  },
  {
    id: 'sessions',
    label: 'Sessions',
    icon: GitBranch,
    heading: 'Every session, one timeline',
    body: 'Browse, resume and fork sessions, and see which branch each one ran on.',
  },
  {
    id: 'changes',
    label: 'Changes',
    icon: GitCompare,
    heading: 'Review what the agent changed',
    body: 'A graphical diff of your working tree. Comment on any line and send it straight back to the agent.',
  },
  {
    id: 'config',
    label: 'Config',
    icon: SlidersHorizontal,
    heading: 'Your harness configuration, readable',
    body: 'Settings, MCP servers and profiles from OMP and PI in one place, with secrets safely masked.',
  },
  {
    id: 'extensions',
    label: 'Extensions',
    icon: Blocks,
    heading: 'Plugins, skills and MCP',
    body: 'Manage plugins, marketplaces, skills and themes, with a dry-run preview before anything changes.',
  },
  {
    id: 'ecosystem',
    label: 'Ecosystem',
    icon: Orbit,
    heading: 'The Harness Useful ecosystem',
    body: 'Dashboard, skills and sidebar at a glance, with live usage and cost from your dashboard.',
  },
  {
    id: 'settings',
    label: 'Settings',
    icon: Settings,
    heading: 'Settings',
    body: 'Theme, motion and startup behavior.',
  },
];

export function viewMeta(id: ViewId): ViewMeta {
  return VIEWS.find((view) => view.id === id) ?? VIEWS[0];
}

/** Ctrl/Cmd digit shortcut for a view, e.g. `mod+3`. */
export function viewShortcut(id: ViewId): string {
  return `mod+${VIEWS.findIndex((view) => view.id === id) + 1}`;
}
