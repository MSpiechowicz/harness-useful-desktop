import Monitor from '@lucide/svelte/icons/monitor';
import Moon from '@lucide/svelte/icons/moon';
import Play from '@lucide/svelte/icons/play';
import Sun from '@lucide/svelte/icons/sun';
import Zap from '@lucide/svelte/icons/zap';

import type { PaletteCommand } from '$lib/components/palette';
import type { MotionPref } from '$lib/ipc/bindings/MotionPref';
import type { ThemePref } from '$lib/ipc/bindings/ThemePref';
import { motion } from '$lib/stores/motion.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { intro } from '../intro/intro.svelte';
import { VIEWS, viewShortcut } from './views';

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

/** Everything the Ctrl/Cmd+K palette can run. Rebuilt reactively because some entries depend on state. */
export function buildCommands(): PaletteCommand[] {
  const navigation: PaletteCommand[] = VIEWS.map((view) => ({
    id: `go-${view.id}`,
    label: `Go to ${view.label}`,
    group: 'Navigate',
    icon: view.icon,
    shortcut: viewShortcut(view.id),
    keywords: view.body,
    disabled: view.id === nav.current,
    run: () => nav.go(view.id),
  }));

  const appearance: PaletteCommand[] = THEMES.map((theme) => ({
    id: `theme-${theme.value}`,
    label: theme.label,
    group: 'Appearance',
    icon: theme.icon,
    keywords: 'color scheme appearance',
    disabled: prefs.value?.theme === theme.value,
    run: () => void prefs.update({ theme: theme.value }),
  }));

  const motionSettings: PaletteCommand[] = MOTIONS.map((setting) => ({
    id: `motion-${setting.value}`,
    label: setting.label,
    group: 'Appearance',
    icon: Zap,
    keywords: 'animation accessibility',
    disabled: prefs.value?.motion === setting.value,
    run: () => void prefs.update({ motion: setting.value }),
  }));

  const replay: PaletteCommand = {
    id: 'replay-intro',
    label: 'Replay intro',
    group: 'Appearance',
    icon: Play,
    keywords: 'animation startup splash',
    run: () => intro.play(motion.level),
  };

  return [...navigation, ...appearance, ...motionSettings, replay];
}
