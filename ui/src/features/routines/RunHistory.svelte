<script lang="ts">
  import History from '@lucide/svelte/icons/history';
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import { untrack } from 'svelte';

  import DataTable from '$lib/components/DataTable.svelte';
  import DocFilter from '$lib/components/docs/DocFilter.svelte';
  import DocSplit from '$lib/components/docs/DocSplit.svelte';
  import DocStage from '$lib/components/docs/DocStage.svelte';
  import { closePanel } from '$lib/components/docs/doc-view';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import FilterRow from '$lib/components/FilterRow.svelte';
  import { ICON } from '$lib/components/icon';
  import Pagination from '$lib/components/Pagination.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import type { Column } from '$lib/components/table';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { RoutineRun } from '$lib/ipc/bindings/RoutineRun';
  import type { RoutineRunsQuery } from '$lib/ipc/bindings/RoutineRunsQuery';
  import { ROUTINE_RUNS_POLL_MS } from '$lib/stores/live';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { autoRefresh } from '$lib/utils/auto-refresh';
  import { clampPage, pageCount } from '$lib/utils/pagination';
  import { formatDateTime, relativeTime } from '$lib/utils/format';

  import { durationText, resultLine, RUN_STATUS_LABELS, RUN_STATUSES, routineTitle, TRIGGER_LABELS } from './routine-format';
  import { runColumns } from './run-columns';
  import { RUNS_PAGE_SIZE, routines, runsKey } from './routines-data.svelte';
  import RunPanel from './RunPanel.svelte';
  import RunStatusBadge from './RunStatusBadge.svelte';

  interface Props {
    /** Answers the question of a run that waits for the user. */
    onopenchat: (run: RoutineRun) => void;
    /** Opens the chat session of a run in Sessions. */
    onopensession: (run: RoutineRun) => void;
    /** Opens the routine a run belongs to in the list of routines. */
    onroutine: (routineId: string) => void;
  }

  let { onopenchat, onopensession, onroutine }: Props = $props();

  let grid: { focusRow: (key: string | number) => void } | undefined = $state();

  const query = $derived<RoutineRunsQuery>({
    routineId: viewState.runsRoutine,
    status: viewState.runsStatus === 'all' ? null : viewState.runsStatus,
    offset: viewState.runsPage * RUNS_PAGE_SIZE,
    limit: RUNS_PAGE_SIZE,
  });
  const page = $derived(routines.runs.data);
  const rows = $derived(page?.runs ?? []);
  const total = $derived(page?.total ?? 0);
  const filtered = $derived(viewState.runsRoutine !== null || viewState.runsStatus !== 'all');
  // Nothing has run yet: one page with the explanation, not an empty table beside a message.
  const none = $derived(page !== null && total === 0 && !filtered);
  const openId = $derived(viewState.runId);
  const live = $derived(rows.find((run) => run.id === openId) ?? null);

  const routineOptions = $derived([
    { value: 'all', label: t('routines.filter.all') },
    ...routines.views.filter((view) => view.routine !== null).map((view) => ({ value: view.id, label: routineTitle(view) })),
  ]);
  const statusOptions = $derived([{ value: 'all' as const, label: t('routines.filter.all') }, ...RUN_STATUSES.map((value) => ({ value, label: t(RUN_STATUS_LABELS[value]) }))]);

  // A panel that closes slides out showing the run it had, not an empty frame.
  let last = $state.raw<RoutineRun | null>(null);
  const shown = $derived(openId === null ? last : live);

  $effect(() => {
    if (live) {
      last = live;
    }
  });

  // Opening the history, turning a page or changing a filter reads again.
  $effect(() => {
    const key = runsKey(query);

    untrack(() => routines.runs.revalidate(key, null));
  });

  // A run that starts or ends while the history is open shows up by itself, and a run that goes on grows in duration.
  $effect(() => {
    void routines.revision;

    untrack(() => void routines.runs.refresh({ quiet: true }));
  });

  $effect(() => autoRefresh(() => routines.runs.refresh({ quiet: true }), ROUTINE_RUNS_POLL_MS, { failing: routines.runs.status === 'error' }));

  // Deleting a routine leaves the pager past the end.
  $effect(() => {
    if (page && total > 0 && viewState.runsPage >= pageCount(total, RUNS_PAGE_SIZE)) {
      viewState.runsPage = clampPage(viewState.runsPage, total, RUNS_PAGE_SIZE);
    }
  });

  // A run that is no longer in the history (its routine was deleted, or a filter leaves it out) is not there to show.
  $effect(() => {
    if (page && openId !== null && live === null) {
      viewState.runId = null;
    }
  });

  function filterRoutine(id: string): void {
    viewState.runsRoutine = id === 'all' ? null : id;
    viewState.runsPage = 0;
  }

  /** Closing a run's panel puts the focus back on its row. */
  function close(): Promise<void> {
    return closePanel(openId, () => (viewState.runId = null), (id) => grid?.focusRow(id));
  }
</script>

<DocStage>
  {#if routines.runs.status === 'error'}
    <div class="grid size-full place-items-center">
      <EmptyState icon={History} title={t('routines.runs.error.title')} body={routines.runs.error?.message} />
    </div>
  {:else if page === null}
    <div class="flex flex-col gap-3 p-8" role="status" aria-label={t('routines.runs.loading')}>
      <Skeleton width="14rem" height="1.75rem" />
      <Skeleton height="2.5rem" />
      <Skeleton height="2.5rem" />
    </div>
  {:else if none}
    <div class="grid size-full place-items-center">
      <EmptyState icon={History} title={t('routines.runs.empty.title')} body={t('routines.runs.empty.body')} />
    </div>
  {:else}
    <DocSplit {openId} emptyIcon={History} emptyMessage={t('routines.runs.detail.empty')} onclose={() => void close()}>
      {#snippet body()}
        <FilterRow count={tn('routines.runs.count', total)}>
          <DocFilter label={t('routines.runs.filter.routine')} options={routineOptions} value={viewState.runsRoutine ?? 'all'} onchange={filterRoutine} clearLabel={t('routines.runs.filter.clearRoutine')} />
          <DocFilter
            label={t('routines.list.column.status')}
            options={statusOptions}
            value={viewState.runsStatus}
            onchange={(next) => {
              viewState.runsStatus = next;
              viewState.runsPage = 0;
            }}
            clearLabel={t('routines.filter.clearStatus')}
          />
        </FilterRow>

        {#if rows.length === 0}
          <p class="min-h-0 flex-1 px-4 py-10 text-center text-sm text-fg-3" role="status">{t('routines.runs.noFilter')}</p>
        {:else}
          <DataTable
            bind:this={grid}
            label={t('routines.runs.label')}
            tableId="routine-runs"
            columns={runColumns()}
            {rows}
            rowKey={(run) => run.id}
            selectedKey={openId}
            sort={{ key: 'time', desc: true }}
            onsort={() => {}}
            onopen={(run) => (viewState.runId = run.id)}
            onmove={(run) => openId !== null && (viewState.runId = run.id)}
            busy={routines.runs.status === 'loading'}
            {cell}
          />

          <Pagination page={viewState.runsPage} pageSize={RUNS_PAGE_SIZE} {total} onpage={(next) => (viewState.runsPage = next)} />
        {/if}
      {/snippet}

      {#snippet panel(inline)}
        <RunPanel
          open={openId !== null}
          {inline}
          run={shown}
          busy={shown !== null && routines.busy.has(shown.id)}
          onstop={() => shown && void routines.stop(shown)}
          onopenchat={() => shown && onopenchat(shown)}
          onopensession={() => shown && onopensession(shown)}
          onroutine={() => shown && onroutine(shown.routineId)}
          onclose={() => void close()}
        />
      {/snippet}
    </DocSplit>
  {/if}
</DocStage>

{#snippet cell(run: RoutineRun, column: Column)}
  {#if column.key === 'time'}
    <span class="tabular text-fg-2" title={formatDateTime(run.startedAt)}>{relativeTime(run.startedAt)}</span>
  {:else if column.key === 'routine'}
    <span class="font-medium text-fg">{run.routineName}</span>
  {:else if column.key === 'status'}
    <RunStatusBadge {run} />
  {:else if column.key === 'duration'}
    <span class="tabular text-fg-2">{run.durationMs === null ? '–' : durationText(run.durationMs)}</span>
  {:else if column.key === 'trigger'}
    <span class="text-fg-2">{t(TRIGGER_LABELS[run.trigger])}</span>
  {:else}
    <span class="flex min-w-0 items-center gap-1.5 text-fg-3" title={resultLine(run)}>
      {#if run.session}<MessageSquare size={ICON.sm} aria-label={t('routines.runs.hasSession')} class="shrink-0" />{/if}
      <span class="truncate">{resultLine(run) || '–'}</span>
    </span>
  {/if}
{/snippet}
