import Download from '@lucide/svelte/icons/download';
import Info from '@lucide/svelte/icons/info';
import FolderClock from '@lucide/svelte/icons/folder-clock';
import Palette from '@lucide/svelte/icons/palette';
import Rocket from '@lucide/svelte/icons/rocket';
import ShieldCheck from '@lucide/svelte/icons/shield-check';
import Terminal from '@lucide/svelte/icons/terminal';

import type { IconComponent } from '$lib/components/icon';
import type { SettingsSectionId } from '$lib/stores/view-state.svelte';

/** The sections of the settings page, in reading order; the section nav and the scroll-spy follow it. */
export const SETTINGS_SECTIONS: readonly { id: SettingsSectionId; label: string; icon: IconComponent; keywords: string }[] = [
  { id: 'appearance', label: 'Appearance', icon: Palette, keywords: 'theme motion dark light animation' },
  { id: 'startup', label: 'Startup', icon: Rocket, keywords: 'intro launch splash' },
  { id: 'harnesses', label: 'Harnesses', icon: Terminal, keywords: 'binary omp pi path setup' },
  { id: 'projects', label: 'Projects', icon: FolderClock, keywords: 'recent folders' },
  { id: 'updates', label: 'Updates', icon: Download, keywords: 'version upgrade release install check new' },
  { id: 'privacy', label: 'Privacy & security', icon: ShieldCheck, keywords: 'data telemetry diagnostics clear' },
  { id: 'about', label: 'About', icon: Info, keywords: 'version build' },
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
