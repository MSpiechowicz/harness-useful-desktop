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
import { t, type MessageKey } from '$lib/i18n/i18n.svelte';
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

const THEMES: { value: ThemePref; label: MessageKey; icon: PaletteCommand['icon'] }[] = [
  { value: 'system', label: 'shell.palette.themeSystem', icon: Monitor },
  { value: 'dark', label: 'shell.palette.themeDark', icon: Moon },
  { value: 'light', label: 'shell.palette.themeLight', icon: Sun },
];

const MOTIONS: { value: MotionPref; label: MessageKey }[] = [
  { value: 'full', label: 'shell.palette.motionFull' },
  { value: 'reduced', label: 'shell.palette.motionReduced' },
  { value: 'off', label: 'shell.palette.motionOff' },
];

export interface PaletteHandlers {
  openShortcuts: () => void;
}

/** Views to jump to, with their shortcuts. */
export function navigationCommands(): PaletteCommand[] {
  return VIEWS.map((view) => ({
    id: `go-${view.id}`,
    label: t('shell.palette.goTo', { view: view.label }),
    group: t('shell.palette.group.navigate'),
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
  const actions = t('shell.palette.group.actions');

  return [
    { id: 'new-chat', label: t('shell.palette.newChat'), group: actions, icon: MessageSquarePlus, shortcut: 'mod+n', keywords: t('shell.palette.newChat.keywords'), run: () => void newChat() },
    {
      id: 'resume-last',
      label: t('shell.palette.resumeLast'),
      group: actions,
      icon: History,
      keywords: t('shell.palette.resumeLast.keywords'),
      disabled: !hasSessions || !project.active,
      run: () => void resumeLastSession(),
    },
    { id: 'jump-chat', label: t('shell.palette.jumpChat'), group: actions, icon: MessageSquare, shortcut: 'mod+0', keywords: t('shell.palette.jumpChat.keywords'), disabled: !hasChat, run: jumpToChat },
    { id: 'rescan-harnesses', label: t('shell.palette.rescan'), group: actions, icon: ScanSearch, keywords: t('shell.palette.rescan.keywords'), disabled: harness.scanning, run: () => void harness.rescan() },
    {
      id: 'refresh-changes',
      label: t('shell.palette.refreshChanges'),
      group: actions,
      icon: RotateCw,
      keywords: t('shell.palette.refreshChanges.keywords'),
      disabled: !project.active,
      run: () => {
        void git.refresh();
        nav.go('changes');
      },
    },
    {
      id: 'refresh-dashboard',
      label: t('shell.palette.refreshDashboard'),
      group: actions,
      icon: ChartNoAxesColumn,
      keywords: t('shell.palette.refreshDashboard.keywords'),
      run: () => {
        void ecosystem.load(true);
        void dashboard.openPanel(true);
        nav.go('ecosystem');
      },
    },
    { id: 'show-shortcuts', label: t('shell.palette.showShortcuts'), group: actions, icon: Keyboard, shortcut: 'shift+?', keywords: t('shell.palette.showShortcuts.keywords'), run: openShortcuts },
    {
      id: 'toggle-sidebar',
      label: ui.railExpanded ? t('shell.palette.collapseSidebar') : t('shell.palette.expandSidebar'),
      group: actions,
      icon: PanelLeft,
      shortcut: 'mod+b',
      keywords: t('shell.palette.sidebar.keywords'),
      run: () => ui.toggleRail(),
    },
    ...SETTINGS_SECTIONS.map<PaletteCommand>((section) => ({
      id: `settings-${section.id}`,
      label: t('shell.palette.settings', { section: section.label }),
      group: actions,
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
  const appearance = t('shell.palette.group.appearance');

  const themes = THEMES.map<PaletteCommand>((theme) => ({
    id: `theme-${theme.value}`,
    label: t(theme.label),
    group: appearance,
    icon: theme.icon,
    keywords: t('shell.palette.theme.keywords'),
    disabled: prefs.value?.theme === theme.value,
    run: () => void prefs.update({ theme: theme.value }),
  }));

  const motions = MOTIONS.map<PaletteCommand>((setting) => ({
    id: `motion-${setting.value}`,
    label: t(setting.label),
    group: appearance,
    icon: Zap,
    keywords: t('shell.palette.motion.keywords'),
    disabled: prefs.value?.motion === setting.value,
    run: () => void prefs.update({ motion: setting.value }),
  }));

  return [
    { id: 'cycle-theme', label: t('shell.palette.toggleTheme'), group: appearance, icon: Sun, keywords: t('shell.palette.toggleTheme.keywords'), run: cycleTheme },
    { id: 'cycle-motion', label: t('shell.palette.toggleMotion'), group: appearance, icon: Zap, keywords: t('shell.palette.toggleMotion.keywords'), run: cycleMotion },
    ...themes,
    ...motions,
    {
      id: 'replay-intro',
      label: t('shell.palette.replayIntro'),
      group: appearance,
      icon: Play,
      keywords: t('shell.palette.replayIntro.keywords'),
      disabled: motion.level === 'off',
      run: () => intro.play(motion.level),
    },
  ];
}
