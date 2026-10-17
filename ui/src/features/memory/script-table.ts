import type { Column } from '$lib/components/table';
import { t } from '$lib/i18n/i18n.svelte';

/** Every column sorts, by the server. Text and dates read from the left, and only the count of runs, a number, from the right. A number or a time starts with the biggest. */
export function scriptColumns(): Column[] {
  return [
    { key: 'name', label: t('memory.scripts.column.name'), sortable: true },
    { key: 'language', label: t('memory.scripts.column.language'), width: '8rem', sortable: true },
    { key: 'scope', label: t('memory.scripts.column.scope'), width: '11rem', from: 'md', sortable: true },
    { key: 'place', label: t('memory.scripts.column.place'), width: '8rem', from: 'lg', sortable: true },
    { key: 'runs', label: t('memory.scripts.column.runs'), width: '7rem', align: 'end', from: 'lg', sortable: true, descFirst: true },
    { key: 'lastRun', label: t('memory.scripts.column.lastRun'), width: '8rem', from: 'lg', sortable: true, descFirst: true },
    { key: 'updated', label: t('memory.scripts.column.updated'), width: '8rem', sortable: true, descFirst: true, from: 'sm' },
  ];
}
