import Download from '@lucide/svelte/icons/download';
import Info from '@lucide/svelte/icons/info';
import FolderClock from '@lucide/svelte/icons/folder-clock';
import Palette from '@lucide/svelte/icons/palette';
import Rocket from '@lucide/svelte/icons/rocket';
import ShieldCheck from '@lucide/svelte/icons/shield-check';
import Terminal from '@lucide/svelte/icons/terminal';

import type { IconComponent } from '$lib/components/icon';
import { t } from '$lib/i18n/i18n.svelte';
import type { SettingsSectionId } from '$lib/stores/view-state.svelte';

/** The sections of the settings page, in reading order; the section nav and the scroll-spy follow it. */
export const SETTINGS_SECTIONS: readonly { id: SettingsSectionId; label: string; icon: IconComponent; keywords: string }[] = [
  {
    id: 'appearance',
    get label() {
      return t('settings.section.appearance');
    },
    icon: Palette,
    get keywords() {
      return t('settings.keywords.appearance');
    },
  },
  {
    id: 'startup',
    get label() {
      return t('settings.section.startup');
    },
    icon: Rocket,
    get keywords() {
      return t('settings.keywords.startup');
    },
  },
  {
    id: 'harnesses',
    get label() {
      return t('settings.section.harnesses');
    },
    icon: Terminal,
    get keywords() {
      return t('settings.keywords.harnesses');
    },
  },
  {
    id: 'projects',
    get label() {
      return t('settings.section.projects');
    },
    icon: FolderClock,
    get keywords() {
      return t('settings.keywords.projects');
    },
  },
  {
    id: 'updates',
    get label() {
      return t('settings.section.updates');
    },
    icon: Download,
    get keywords() {
      return t('settings.keywords.updates');
    },
  },
  {
    id: 'privacy',
    get label() {
      return t('settings.section.privacy');
    },
    icon: ShieldCheck,
    get keywords() {
      return t('settings.keywords.privacy');
    },
  },
  {
    id: 'about',
    get label() {
      return t('settings.section.about');
    },
    icon: Info,
    get keywords() {
      return t('settings.keywords.about');
    },
  },
];

export interface SectionBox {
  id: SettingsSectionId;
  top: number;
}

/**
 * The section being read: the last one whose top has passed the reading line, or the last of all
 * once the page is scrolled to its end (the final sections are shorter than the viewport).
 */
export function activeSection(boxes: readonly SectionBox[], readingLine: number, atEnd: boolean): SettingsSectionId | null {
  if (boxes.length === 0) {
    return null;
  }

  if (atEnd) {
    return boxes[boxes.length - 1].id;
  }

  const passed = boxes.filter((box) => box.top <= readingLine);

  return (passed[passed.length - 1] ?? boxes[0]).id;
}
