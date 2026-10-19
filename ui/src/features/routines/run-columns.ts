import type { Column } from '$lib/components/table';
import { t } from '$lib/i18n/i18n.svelte';

/** The columns of the run history. The server orders it, newest first, so none sorts. The routine's name takes what the others leave. */
export function runColumns(): Column[] {
  return [
    { key: 'time', label: t('routines.runs.column.time'), width: '9rem' },
    { key: 'routine', label: t('routines.runs.column.routine') },
    { key: 'status', label: t('routines.runs.column.status'), width: '10.5rem' },
    { key: 'duration', label: t('routines.runs.column.duration'), width: '7rem', align: 'end', from: 'sm' },
    { key: 'trigger', label: t('routines.runs.column.trigger'), width: '9rem', from: 'md' },
    { key: 'result', label: t('routines.runs.column.result'), width: '18rem', from: 'lg' },
  ];
}
