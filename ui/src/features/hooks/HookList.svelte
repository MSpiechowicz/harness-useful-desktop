<script lang="ts">
  import DocGroupHeading from '$lib/components/docs/DocGroupHeading.svelte';
  import DocSwitchCell from '$lib/components/docs/DocSwitchCell.svelte';
  import DocTable from '$lib/components/docs/DocTable.svelte';
  import DocTeamNote from '$lib/components/docs/DocTeamNote.svelte';
  import type { DocListHandlers } from '$lib/components/docs/doc-groups';
  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HookView } from '$lib/ipc/bindings/HookView';
  import { cx } from '$lib/utils/cx';

  import { ACTION_LABELS, EVENT_ICONS, EVENT_LABELS, hookTitle, type HookGroup, MAX_PER_SCOPE, SCOPE_ICONS, SCOPE_LABELS, viewKey } from './hook-format';
  import { switchLock, teamSwitchedOn } from './hook-team';
  import HookStatusBadge from './HookStatusBadge.svelte';

  interface Props extends DocListHandlers<HookView> {
    groups: HookGroup[];
    /** The hook whose panel is open. */
    openId: string | null;
    /** The name of the open project, which the project group is titled with. */
    projectName: string | null;
    /** Why the latest refresh of the team hooks failed, said under the Team heading; null when it did not. */
    teamProblem?: string | null;
    /** Team hooks run without local approval (effective trust, whatever the Settings show): said as a badge in the Team heading. */
    teamTrusted?: boolean;
    /** Hooks a change is being written for (by `viewKey`): their switch waits. */
    busy: ReadonlySet<string>;
  }

  let { groups, openId, projectName, teamProblem = null, teamTrusted = false, busy, onopen, onmove, ontoggle, onadd, table = $bindable() }: Props = $props();

  const labels = $derived({
    global: t(SCOPE_LABELS.global),
    project: t(SCOPE_LABELS.project),
    invalid: t('hooks.group.invalid'),
    team: t('hooks.group.team'),
    trusted: t('hooks.list.teamTrusted'),
    addGlobal: t('hooks.list.addGlobal'),
    addProject: t('hooks.list.addProject'),
    count: (total: number) => t('hooks.list.groupCount', { count: total, max: MAX_PER_SCOPE }),
  });

  const COLUMNS = $derived([
    { label: t('hooks.list.column.hook') },
    { label: t('hooks.list.column.when'), col: 'w-[17rem]' },
    { label: t('hooks.list.column.does'), col: 'hidden w-[8.5rem] @min-[780px]:table-column', head: 'hidden @min-[780px]:table-cell' },
    { label: t('hooks.list.column.status'), col: 'w-[10.5rem]' },
    { label: t('hooks.list.column.on'), col: 'w-[5.5rem]', right: true },
  ]);

  const span = $derived(COLUMNS.length + 1);

  /** Why a row's switch is off, or null when it works: a hook that waits for approval or cannot run is never turned on by a click, as that would approve what the user has not seen. */
  function lockReason(view: HookView): string | null {
    const key = switchLock(view);

    return key === null ? null : t(key);
  }

  /** Whether the row's label offers to turn it off: a team hook as this computer has it, a local one as its file says. */
  function isOn(view: HookView): boolean {
    return view.origin === 'team' ? teamSwitchedOn(view) : (view.hook?.enabled ?? false);
  }

  /** The switch position of a row: a local hook as its file says, a team hook as this computer has it. */
  function switchedOn(view: HookView): boolean {
    return view.origin === 'team' ? teamSwitchedOn(view) : view.status !== 'pending' && view.status !== 'invalid' && (view.hook?.enabled ?? false);
  }
</script>

{#snippet heading(group: HookGroup)}
  <DocGroupHeading {group} {span} {projectName} {teamTrusted} {labels} icons={SCOPE_ICONS} {onadd} />
{/snippet}

{#snippet note(group: HookGroup)}
  {#if group.id === 'team' && teamProblem}<DocTeamNote {span} text={t('hooks.list.teamProblem', { reason: teamProblem })} />{/if}
{/snippet}

{#snippet cells(view: HookView)}
  {@const hook = view.hook}
  {@const key = viewKey(view)}
  {@const WhenIcon = hook ? EVENT_ICONS[hook.event] : null}

  <td role="gridcell" class="border-b border-line/60 px-3">
    <div class={cx('truncate font-medium', view.status === 'off' ? 'text-fg-2' : 'text-fg')} title={hookTitle(view)}>{hookTitle(view)}</div>
  </td>

  <td role="gridcell" class="border-b border-line/60 px-3 text-fg-2">
    {#if hook && WhenIcon}
      <div class="flex min-w-0 items-center gap-1.5">
        <WhenIcon size={ICON.sm} aria-hidden="true" class="shrink-0 text-fg-3" />
        <span class="truncate">{t(EVENT_LABELS[hook.event])}</span>
        {#if hook.matcher}<span class="h-5 max-w-[7rem] truncate rounded-sm bg-surface-3 px-1.5 font-mono text-xs leading-5 text-fg-2" title={t('hooks.field.toolFilter')}>{hook.matcher}</span>{/if}
      </div>
    {:else}
      <span class="text-fg-3">–</span>
    {/if}
  </td>

  <td role="gridcell" class="hidden border-b border-line/60 px-3 text-fg-2 @min-[780px]:table-cell">
    <div class="truncate">{#if hook}{t(ACTION_LABELS[hook.action.type])}{:else}–{/if}</div>
  </td>

  <td role="gridcell" class="border-b border-line/60 px-3"><HookStatusBadge status={view.status} /></td>

  <td role="gridcell" class="border-b border-line/60 px-3 text-right">
    <DocSwitchCell inRow label={t(isOn(view) ? 'hooks.switch.off' : 'hooks.switch.on', { name: hookTitle(view) })} checked={switchedOn(view)} lockReason={lockReason(view)} busy={busy.has(key)} onchange={(next) => ontoggle(view, next)} />
  </td>
{/snippet}

{#snippet empty(group: HookGroup)}
  {#if group.id === 'project' && !projectName}
    {t('hooks.list.noProject')}
  {:else}
    {t(group.id === 'team' ? 'hooks.list.teamEmpty' : 'hooks.list.groupEmpty')}
  {/if}
{/snippet}

<DocTable bind:this={table} label={t('hooks.list.label')} columns={COLUMNS} {groups} rowKey={viewKey} {openId} {onopen} {onmove} {heading} {note} {cells} {empty} />
