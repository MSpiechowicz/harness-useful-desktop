import { Folded } from '$lib/utils/folded.svelte';

import type { MessageKey } from '$lib/i18n/i18n.svelte';

export const SECTION_IDS = ['today', 'week', 'older'] as const;
export type SectionId = (typeof SECTION_IDS)[number];

export const SECTION_LABELS: Record<SectionId, MessageKey> = {
  today: 'sessions.section.today',
  week: 'sessions.section.week',
  older: 'sessions.section.older',
};

const DAY = 24 * 60 * 60 * 1000;

/** Which heading a session sits under: today, the six days before it, or older (a time that cannot be read is older). */
export function sectionOf(iso: string, now: number = Date.now()): SectionId {
  const time = Date.parse(iso);
  const startOfToday = new Date(now).setHours(0, 0, 0, 0);

  if (Number.isNaN(time)) {
    return 'older';
  }

  if (time >= startOfToday) {
    return 'today';
  }

  return time >= startOfToday - 6 * DAY ? 'week' : 'older';
}

/** The headings the user folded away; kept outside the view so the choice survives a visit to another view. */
export const folded = new Folded<SectionId>();
