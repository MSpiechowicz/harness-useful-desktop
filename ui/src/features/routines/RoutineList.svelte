<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import DocScopeCell from '$lib/components/docs/DocScopeCell.svelte';
  import { docScope } from '$lib/components/docs/doc-scope';
  import DocSwitchCell from '$lib/components/docs/DocSwitchCell.svelte';
  import DocTable from '$lib/components/docs/DocTable.svelte';
  import type { DocTableHandle } from '$lib/components/docs/doc-groups';
  import { ICON } from '$lib/components/icon';
  import type { Sort } from '$lib/components/table';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';
  import type { RoutineView } from '$lib/ipc/bindings/RoutineView';
  import { cx } from '$lib/utils/cx';

  import { ACTION_ICONS, doesText, isQueued, routineDoc, routineTitle, untilTime, viewKey, WHEN_ICONS, whenText } from './routine-format';
  import { switchedOn, switchLock, teamProject } from './routine-team';
  import RoutineStatusBadge from './RoutineStatusBadge.svelte';

  interface Props {
    /** The routines of the page on screen. */
    rows: RoutineView[];
    /** The pager under the rows: the routines are paged here, on the client. */
    pagination: { page: number; pageSize: number; total: number; onpage: (page: number) => void };
    /** The sort the routines are in; null for the run order. */
    sort: Sort | null;
    onsort: (sort: Sort) => void;
    /** The routine whose panel is open. */
    openId: string | null;
    /** The projects on this computer, which a routine's Scope is named by and a team routine is turned on in. */
    projects: ProjectScope[];
    /** Routines a change is being written for (by `viewKey`): their switch waits. */
    busy: ReadonlySet<string>;
    /** The routines with a run that asks the user something, by routine id. */
    waiting: ReadonlyMap<string, string>;
    onopen: (id: string) => void;
    /** Moves the open panel along when the arrow keys move the focus. */
    onmove: (id: string) => void;
    ontoggle: (view: RoutineView, enabled: boolean) => void;
    /** The table, for the view to put the focus back on a row once its panel is closed. */
    table?: DocTableHandle;
  }

  let { rows, pagination, sort, onsort, openId, projects, busy, waiting, onopen, onmove, ontoggle, table = $bindable() }: Props = $props();

  const COLUMNS = $derived([
    { key: 'routine', label: t('routines.list.column.routine') },
    { key: 'scope', label: t('routines.list.column.scope'), col: 'w-[9rem]' },
    { key: 'when', label: t('routines.list.column.when'), col: 'w-[13rem]' },
    { key: 'next', label: t('routines.list.column.next'), col: 'hidden w-[8.5rem] @min-[960px]:table-column', head: 'hidden @min-[960px]:table-cell' },
    { key: 'status', label: t('routines.list.column.status'), col: 'w-[10.5rem]' },
    { key: 'on', label: t('routines.list.column.on'), col: 'w-[5.5rem]', right: true, descFirst: true },
  ]);

  /** Why a row's switch is locked, or null while it works. Turning on a routine that waits for approval would approve it unseen, so a click never does. */
  function lockReason(view: RoutineView): string | null {
    const key = switchLock(view, teamProject(view, projects) !== null);

    return key === null ? null : t(key);
  }
</script>

{#snippet cells(view: RoutineView)}
  {@const routine = view.routine}
  {@const key = viewKey(view)}
  {@const DoesIcon = routine ? ACTION_ICONS[routine.action.type] : null}
  {@const WhenIcon = routine ? WHEN_ICONS[routine.when.type] : null}

  <td role="gridcell" class="border-b border-line/60 px-3">
    <div class="flex min-w-0 items-center gap-2">
      {#if routine && DoesIcon}
        <span class="shrink-0 text-fg-3" title={doesText(routine.action)}><DoesIcon size={ICON.sm} aria-hidden="true" /></span>
      {/if}
      <span class={cx('truncate font-medium', view.status === 'off' ? 'text-fg-2' : 'text-fg')} title={routineTitle(view)}>{routineTitle(view)}</span>
      {#if view.runsUnattended}<Badge tone="warning" size="lg" class="shrink-0">{t('routines.list.unattended')}</Badge>{/if}
    </div>
  </td>

  <DocScopeCell scope={docScope(routineDoc(view), projects)} remote={view.origin === 'team'} />

  <td role="gridcell" class="border-b border-line/60 px-3 text-fg-2">
    {#if routine && WhenIcon}
      <div class="flex min-w-0 items-center gap-1.5">
        <WhenIcon size={ICON.sm} aria-hidden="true" class="shrink-0 text-fg-3" />
        <span class={cx('truncate', routine.when.type === 'cron' && 'font-mono text-xs')} title={whenText(routine.when)}>{whenText(routine.when)}</span>
      </div>
    {:else}
      <span class="text-fg-3">–</span>
    {/if}
  </td>

  <td role="gridcell" class="hidden border-b border-line/60 px-3 text-fg-2 @min-[960px]:table-cell">
    <div class="truncate">{#if view.nextRun}{untilTime(view.nextRun)}{:else}–{/if}</div>
  </td>

  <td role="gridcell" class="border-b border-line/60 px-3"><RoutineStatusBadge status={view.status} needsYou={waiting.has(view.id)} queued={isQueued(view)} problem={view.problem} /></td>

  <td role="gridcell" class="border-b border-line/60 px-3 text-right">
    <DocSwitchCell inRow label={t(switchedOn(view) ? 'routines.switch.off' : 'routines.switch.on', { name: routineTitle(view) })} checked={switchedOn(view)} lockReason={lockReason(view)} busy={busy.has(key)} onchange={(next) => ontoggle(view, next)} />
  </td>
{/snippet}

<DocTable bind:this={table} label={t('routines.list.label')} columns={COLUMNS} {sort} {onsort} {rows} {pagination} rowKey={viewKey} {openId} {onopen} {onmove} {cells} />
