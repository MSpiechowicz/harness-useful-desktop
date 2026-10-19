<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import DocGroupHeading from '$lib/components/docs/DocGroupHeading.svelte';
  import DocSwitchCell from '$lib/components/docs/DocSwitchCell.svelte';
  import DocTable from '$lib/components/docs/DocTable.svelte';
  import DocTeamNote from '$lib/components/docs/DocTeamNote.svelte';
  import type { DocListHandlers } from '$lib/components/docs/doc-groups';
  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { RoutineView } from '$lib/ipc/bindings/RoutineView';
  import { cx } from '$lib/utils/cx';

  import { ACTION_ICONS, doesText, isQueued, MAX_PER_SCOPE, routineTitle, type RoutineGroup, SCOPE_ICONS, SCOPE_LABELS, untilTime, viewKey, WHEN_ICONS, whenText } from './routine-format';
  import { switchLock, teamSwitchedOn } from './routine-team';
  import RoutineStatusBadge from './RoutineStatusBadge.svelte';

  interface Props extends DocListHandlers<RoutineView> {
    groups: RoutineGroup[];
    /** The routine whose panel is open. */
    openId: string | null;
    /** The name of the open project, which the project group is titled with. */
    projectName: string | null;
    /** Why the latest refresh of the team routines failed, said under the Team heading; null when it did not. */
    teamProblem?: string | null;
    /** Team routines run without local approval (effective trust, whatever the Settings show): said as a badge in the Team heading. */
    teamTrusted?: boolean;
    /** Routines a change is being written for (by `viewKey`): their switch waits. */
    busy: ReadonlySet<string>;
    /** The routines with a run that asks the user something, by routine id. */
    waiting: ReadonlyMap<string, string>;
  }

  let { groups, openId, projectName, teamProblem = null, teamTrusted = false, busy, waiting, onopen, onmove, ontoggle, onadd, table = $bindable() }: Props = $props();

  const labels = $derived({
    global: t(SCOPE_LABELS.global),
    project: t(SCOPE_LABELS.project),
    invalid: t('routines.group.invalid'),
    team: t('routines.group.team'),
    trusted: t('routines.list.teamTrusted'),
    addGlobal: t('routines.list.addGlobal'),
    addProject: t('routines.list.addProject'),
    count: (total: number) => t('routines.list.groupCount', { count: total, max: MAX_PER_SCOPE }),
  });

  const COLUMNS = $derived([
    { label: t('routines.list.column.routine') },
    { label: t('routines.list.column.when'), col: 'w-[15rem]' },
    { label: t('routines.list.column.next'), col: 'hidden w-[8.5rem] @min-[780px]:table-column', head: 'hidden @min-[780px]:table-cell' },
    { label: t('routines.list.column.status'), col: 'w-[10.5rem]' },
    { label: t('routines.list.column.on'), col: 'w-[5.5rem]', right: true },
  ]);

  const span = $derived(COLUMNS.length + 1);

  /** Why a row's switch is locked, or null while it works. Turning on a routine that waits for approval would approve it unseen, so a click never does. */
  function lockReason(view: RoutineView): string | null {
    const key = switchLock(view, projectName !== null);

    return key === null ? null : t(key);
  }

  /** The switch position of a row: a local routine as its file says, a team routine as this computer has it. */
  function switchedOn(view: RoutineView): boolean {
    return view.origin === 'team' ? teamSwitchedOn(view) : view.status !== 'pending' && view.status !== 'invalid' && (view.routine?.enabled ?? false);
  }
</script>

{#snippet heading(group: RoutineGroup)}
  <DocGroupHeading {group} {span} {projectName} {teamTrusted} {labels} icons={SCOPE_ICONS} {onadd} />
{/snippet}

{#snippet note(group: RoutineGroup)}
  {#if group.id === 'team' && teamProblem}<DocTeamNote {span} text={t('routines.list.teamProblem', { reason: teamProblem })} />{/if}
{/snippet}

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
      {#if view.runsUnattended}<Badge tone="warning" class="shrink-0">{t('routines.list.unattended')}</Badge>{/if}
    </div>
  </td>

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

  <td role="gridcell" class="hidden border-b border-line/60 px-3 text-fg-2 @min-[780px]:table-cell">
    <div class="truncate">{#if view.nextRun}{untilTime(view.nextRun)}{:else}–{/if}</div>
  </td>

  <td role="gridcell" class="border-b border-line/60 px-3"><RoutineStatusBadge status={view.status} needsYou={waiting.has(view.id)} queued={isQueued(view)} problem={view.problem} /></td>

  <td role="gridcell" class="border-b border-line/60 px-3 text-right">
    <DocSwitchCell inRow label={t(switchedOn(view) ? 'routines.switch.off' : 'routines.switch.on', { name: routineTitle(view) })} checked={switchedOn(view)} lockReason={lockReason(view)} busy={busy.has(key)} onchange={(next) => ontoggle(view, next)} />
  </td>
{/snippet}

{#snippet empty(group: RoutineGroup)}
  {#if group.id === 'project' && !projectName}
    {t('routines.list.noProject')}
  {:else}
    {t(group.id === 'team' ? 'routines.list.teamEmpty' : 'routines.list.groupEmpty')}
  {/if}
{/snippet}

<DocTable bind:this={table} label={t('routines.list.label')} columns={COLUMNS} {groups} rowKey={viewKey} {openId} {onopen} {onmove} {heading} {note} {cells} {empty} />
