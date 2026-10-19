<script lang="ts">
  import CalendarClock from '@lucide/svelte/icons/calendar-clock';

  import { t } from '$lib/i18n/i18n.svelte';
  import type { SessionRoutineTag } from '$lib/ipc/bindings/SessionRoutineTag';
  import { cx } from '$lib/utils/cx';

  import { showRoutine } from './routine-open';

  interface Props {
    routine: Pick<SessionRoutineTag, 'routineId' | 'name'>;
    class?: string;
  }

  let { routine, class: className }: Props = $props();

  const label = $derived(t('routines.chip', { name: routine.name }));
</script>

<!-- "Routine · Morning review": the chat or the session was started by a routine. It is a button, a click shows the routine. -->
<button
  type="button"
  title={t('routines.chip.show', { name: routine.name })}
  class={cx(
    'pointer-events-auto inline-flex min-w-0 items-center gap-1.5 rounded-full bg-accent-wash px-2 py-0.5 text-xs leading-4 font-medium tracking-wide whitespace-nowrap text-accent-text transition-[background-color,transform] duration-[var(--dur-fast)] ease-[var(--ease-out-expo)] hover:bg-accent/20 active:scale-[0.97]',
    className,
  )}
  onclick={() => showRoutine(routine.routineId)}
>
  <CalendarClock size={12} class="shrink-0" aria-hidden="true" />
  <span class="min-w-0 truncate">{label}</span>
</button>
