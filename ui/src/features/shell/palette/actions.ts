import ChartNoAxesColumn from '@lucide/svelte/icons/chart-no-axes-column';
import Keyboard from '@lucide/svelte/icons/keyboard';
import MessageSquarePlus from '@lucide/svelte/icons/message-square-plus';
import Monitor from '@lucide/svelte/icons/monitor';
import Moon from '@lucide/svelte/icons/moon';
import PanelLeft from '@lucide/svelte/icons/panel-left';
import Play from '@lucide/svelte/icons/play';
import RotateCw from '@lucide/svelte/icons/rotate-cw';
import ScanSearch from '@lucide/svelte/icons/scan-search';
import Sun from '@lucide/svelte/icons/sun';
import Zap from '@lucide/svelte/icons/zap';
import History from '@lucide/svelte/icons/history';
import MessageSquare from '@lucide/svelte/icons/message-square';

import type { PaletteCommand } from '$lib/components/palette';
import type { MotionPref } from '$lib/ipc/bindings/MotionPref';
import type { ThemePref } from '$lib/ipc/bindings/ThemePref';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { dashboard } from '$lib/stores/dashboard.svelte';
import { ecosystem } from '$lib/stores/ecosystem.svelte';
import { git } from '$lib/stores/git.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { motion } from '$lib/stores/motion.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';
import { sessionList } from '$lib/stores/sessions.svelte';
import { ui } from '$lib/stores/ui-state.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { intro } from '../../intro/intro-state.svelte';
import { SETTINGS_SECTIONS } from '../../settings/sections';
import { cycleMotion, cycleTheme, jumpToChat, newChat, resumeLastSession } from '../actions';
import { VIEWS, viewShortcut } from '../views';

const THEMES: { value: ThemePref; label: string; icon: PaletteCommand['icon'] }[] = [
  { value: 'system', label: 'Theme: Follow system', icon: Monitor },
  { value: 'dark', label: 'Theme: Dark', icon: Moon },
  { value: 'light', label: 'Theme: Light', icon: Sun },
];

const MOTIONS: { value: MotionPref; label: string }[] = [
  { value: 'full', label: 'Motion: Full' },
  { value: 'reduced', label: 'Motion: Reduced' },
  { value: 'off', label: 'Motion: Off' },
];

export interface PaletteHandlers {
  openShortcuts: () => void;
}

/** Views to jump to, with their shortcuts. */
export function navigationCommands(): PaletteCommand[] {
  return VIEWS.map((view) => ({
    id: `go-${view.id}`,
    label: `Go to ${view.label}`,
    group: 'Navigate',
    kind: 'nav',
    icon: view.icon,
    shortcut: viewShortcut(view.id),
    keywords: view.body,
    disabled: view.id === nav.current,
    run: () => nav.go(view.id),
  }));
}

/** Things to do: chats, harness upkeep, layout, the settings sections. */
export function actionCommands({ openShortcuts }: PaletteHandlers): PaletteCommand[] {
  const hasChat = agentSessions.active !== null;
  const hasSessions = (sessionList.data?.length ?? 0) > 0;

  return [
    { id: 'new-chat', label: 'New chat', group: 'Actions', icon: MessageSquarePlus, shortcut: 'mod+n', keywords: 'start conversation agent', run: () => void newChat() },
    {
      id: 'resume-last',
      label: 'Resume last session',
      group: 'Actions',
      icon: History,
      keywords: 'continue previous conversation',
      disabled: !hasSessions || !project.active,
      run: () => void resumeLastSession(),
    },
    { id: 'jump-chat', label: 'Jump to active chat', group: 'Actions', icon: MessageSquare, shortcut: 'mod+0', keywords: 'composer focus', disabled: !hasChat, run: jumpToChat },
    { id: 'rescan-harnesses', label: 'Re-scan harnesses', group: 'Actions', icon: ScanSearch, keywords: 'detect omp pi install refresh', disabled: harness.scanning, run: () => void harness.rescan() },
    {
      id: 'refresh-changes',
      label: 'Refresh changes',
      group: 'Actions',
      icon: RotateCw,
      keywords: 'git status diff reload',
      disabled: !project.active,
      run: () => {
        void git.refresh();
        nav.go('changes');
      },
    },
    {
      id: 'refresh-dashboard',
      label: 'Ecosystem: refresh dashboard',
      group: 'Actions',
      icon: ChartNoAxesColumn,
      keywords: 'usage cost tokens spend companions reload',
      run: () => {
        void ecosystem.load(true);
        void dashboard.openPanel(true);
        nav.go('ecosystem');
      },
    },
    { id: 'show-shortcuts', label: 'Show keyboard shortcuts', group: 'Actions', icon: Keyboard, shortcut: 'shift+?', keywords: 'keys help cheatsheet', run: openShortcuts },
    { id: 'toggle-sidebar', label: ui.railExpanded ? 'Collapse the sidebar' : 'Expand the sidebar', group: 'Actions', icon: PanelLeft, shortcut: 'mod+b', keywords: 'rail labels navigation', run: () => ui.toggleRail() },
    ...SETTINGS_SECTIONS.map<PaletteCommand>((section) => ({
      id: `settings-${section.id}`,
      label: `Settings: ${section.label}`,
      group: 'Actions',
      icon: section.icon,
      keywords: section.keywords,
      run: () => {
        viewState.settingsTarget = section.id;
        nav.go('settings');
      },
    })),
  ];
}

/** Theme, motion and the intro. */
export function appearanceCommands(): PaletteCommand[] {
  const themes = THEMES.map<PaletteCommand>((theme) => ({
    id: `theme-${theme.value}`,
    label: theme.label,
    group: 'Appearance',
    icon: theme.icon,
    keywords: 'color scheme appearance',
    disabled: prefs.value?.theme === theme.value,
    run: () => void prefs.update({ theme: theme.value }),
  }));

  const motions = MOTIONS.map<PaletteCommand>((setting) => ({
    id: `motion-${setting.value}`,
    label: setting.label,
    group: 'Appearance',
    icon: Zap,
    keywords: 'animation accessibility',
    disabled: prefs.value?.motion === setting.value,
    run: () => void prefs.update({ motion: setting.value }),
  }));

  return [
    { id: 'cycle-theme', label: 'Toggle theme', group: 'Appearance', icon: Sun, keywords: 'dark light system switch', run: cycleTheme },
    { id: 'cycle-motion', label: 'Toggle motion', group: 'Appearance', icon: Zap, keywords: 'animation reduce full off', run: cycleMotion },
    ...themes,
    ...motions,
    { id: 'replay-intro', label: 'Replay intro', group: 'Appearance', icon: Play, keywords: 'animation startup splash', disabled: motion.level === 'off', run: () => intro.play(motion.level) },
  ];
}
