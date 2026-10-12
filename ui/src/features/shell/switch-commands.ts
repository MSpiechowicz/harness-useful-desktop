import Folder from '@lucide/svelte/icons/folder';
import FolderOpen from '@lucide/svelte/icons/folder-open';
import FolderX from '@lucide/svelte/icons/folder-x';
import Replace from '@lucide/svelte/icons/replace';
import Wand from '@lucide/svelte/icons/wand-sparkles';

import type { PaletteCommand } from '$lib/components/palette';
import { t } from '$lib/i18n/i18n.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { project } from '$lib/stores/project.svelte';

import { onboarding } from '../onboarding/onboarding-state.svelte';

/** Palette entries for switching harness and project, and for re-running setup. */
export function buildSwitchCommands(): PaletteCommand[] {
  const harnesses: PaletteCommand[] = harness.summaries.map((summary) => ({
    id: `switch-harness-${summary.info.id}`,
    label: t('shell.palette.switchHarness', { name: summary.info.shortName }),
    group: t('shell.palette.group.actions'),
    icon: Replace,
    keywords: `${summary.info.name} ${t('shell.palette.switchHarness.keywords')}`,
    disabled: !summary.detection.installed || harness.active?.info.id === summary.info.id,
    run: () => void harness.select(summary.info.id),
  }));

  const open: PaletteCommand = {
    id: 'open-project',
    label: t('shell.palette.openProject'),
    group: t('shell.palette.group.actions'),
    icon: FolderOpen,
    keywords: t('shell.palette.openProject.keywords'),
    run: () => void project.pick(),
  };

  const recents: PaletteCommand[] = project.recents.map((recent) => ({
    id: `open-recent-${recent.path}`,
    label: t('shell.palette.openRecent', { name: recent.name }),
    group: t('shell.palette.group.actions'),
    icon: Folder,
    keywords: recent.path,
    disabled: !recent.exists || project.active?.path === recent.path,
    run: () => void project.open(recent.path),
  }));

  const close: PaletteCommand[] = project.active
    ? [
        {
          id: 'close-project',
          label: t('shell.palette.closeProject'),
          group: t('shell.palette.group.actions'),
          icon: FolderX,
          keywords: t('shell.palette.closeProject.keywords'),
          run: () => void project.close(),
        },
      ]
    : [];

  const setup: PaletteCommand = {
    id: 'run-onboarding',
    label: t('shell.palette.onboarding'),
    group: t('shell.palette.group.actions'),
    icon: Wand,
    keywords: t('shell.palette.onboarding.keywords'),
    run: () => onboarding.restart(),
  };

  return [...harnesses, open, ...recents, ...close, setup];
}
