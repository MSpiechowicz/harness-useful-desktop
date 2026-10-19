<script lang="ts">
  import { untrack } from 'svelte';

  import { t } from '$lib/i18n/i18n.svelte';
  import { ipc } from '$lib/ipc/client';
  import { ViewLoader } from '$lib/stores/view-loader.svelte';
  import { relativeTime } from '$lib/utils/format';
  import { formatDateTime } from '$lib/utils/format';
  import type { RoutineRunsPage } from '$lib/ipc/bindings/RoutineRunsPage';

  import { durationText, TRIGGER_LABELS } from './routine-format';
  import { routines } from './routines-data.svelte';
  import RunStatusBadge from './RunStatusBadge.svelte';
  
  interface Props {
    routineId: string;
    /** Opens a run in the run history. */
    onopen: (runId: string) => void;
    /** Opens the whole history of the routine. */
    onall: () => void;
  }

  /** How many of the latest runs the panel lists. */
  const SHOWN = 3;

  let { routineId, onopen, onall }: Props = $props();

  const recent = new ViewLoader<RoutineRunsPage, string>((id) => ipc.invoke('routine_runs', { query: { routineId: id, status: null, offset: 0, limit: SHOWN } }));

  // The latest runs of the routine on screen: another routine, or a run that starts or ends, reads again.
  $effect(() => {
    void routines.revision;
    const id = routineId;

    untrack(() => recent.revalidate(id, null));
  });

  const page = $derived(recent.data);
</script>

{#if page === null}
  <p class="text-sm text-fg-3">…</p>
{:else if page.runs.length === 0}
  <p class="text-sm text-fg-3">{t('routines.panel.runs.none')}</p>
{:else}
  <ul class="flex flex-col" aria-label={t('routines.panel.runs')}>
    {#each page.runs as run (run.id)}
      <li>
        <button type="button" class="flex w-full items-center gap-3 rounded-sm px-1 py-1.5 text-left text-sm transition-colors duration-[var(--dur-fast)] hover:bg-hover" onclick={() => onopen(run.id)}>
          <RunStatusBadge {run} />
          <span class="min-w-0 flex-1 truncate text-fg-2" title={formatDateTime(run.startedAt)}>{relativeTime(run.startedAt)} · {t(TRIGGER_LABELS[run.trigger])}</span>
          {#if run.durationMs !== null}<span class="tabular shrink-0 text-fg-3">{durationText(run.durationMs)}</span>{/if}
        </button>
      </li>
    {/each}
  </ul>

  {#if page.total > page.runs.length}
    <button type="button" class="mt-1 w-fit text-sm text-accent-text hover:underline" onclick={onall}>{t('routines.panel.runs.all', { count: page.total })}</button>
  {/if}
{/if}
