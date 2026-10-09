import Folder from '@lucide/svelte/icons/folder';
import FolderOpen from '@lucide/svelte/icons/folder-open';
import FolderX from '@lucide/svelte/icons/folder-x';
import Replace from '@lucide/svelte/icons/replace';
import Wand from '@lucide/svelte/icons/wand-sparkles';

import type { PaletteCommand } from '$lib/components/palette';
import { harness } from '$lib/stores/harness.svelte';
import { project } from '$lib/stores/project.svelte';

import { onboarding } from '../onboarding/onboarding.svelte';

/** Palette entries for switching harness and project, and for re-running setup. */
export function buildSwitchCommands(): PaletteCommand[] {
  const harnesses: PaletteCommand[] = harness.summaries.map((summary) => ({
    id: `switch-harness-${summary.info.id}`,
    label: `Switch harness: ${summary.info.shortName}`,
    group: 'Actions',
    icon: Replace,
    keywords: `${summary.info.name} agent cli`,
    disabled: !summary.detection.installed || harness.active?.info.id === summary.info.id,
    run: () => void harness.select(summary.info.id),
  }));

  const open: PaletteCommand = {
    id: 'open-project',
    label: 'Open project…',
    group: 'Actions',
    icon: FolderOpen,
    keywords: 'folder directory pick',
    run: () => void project.pick(),
  };

  const recents: PaletteCommand[] = project.recents.map((recent) => ({
    id: `open-recent-${recent.path}`,
    label: `Open recent: ${recent.name}`,
    group: 'Actions',
    icon: Folder,
    keywords: recent.path,
    disabled: !recent.exists || project.active?.path === recent.path,
    run: () => void project.open(recent.path),
  }));

  const close: PaletteCommand[] = project.active
    ? [{ id: 'close-project', label: 'Close project', group: 'Actions', icon: FolderX, keywords: 'folder', run: () => void project.close() }]
    : [];

  const setup: PaletteCommand = {
    id: 'run-onboarding',
    label: 'Run onboarding again',
    group: 'Actions',
    icon: Wand,
    keywords: 'setup detect welcome',
    run: () => onboarding.restart(),
  };

  return [...harnesses, open, ...recents, ...close, setup];
}
