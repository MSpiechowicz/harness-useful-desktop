<script lang="ts">
  import DocScopeCell from '$lib/components/docs/DocScopeCell.svelte';
  import DocSwitchCell from '$lib/components/docs/DocSwitchCell.svelte';
  import DocTable from '$lib/components/docs/DocTable.svelte';
  import type { DocListHandlers } from '$lib/components/docs/doc-groups';
  import type { DocScope } from '$lib/components/docs/doc-scope';
  import { ICON } from '$lib/components/icon';
  import type { Sort } from '$lib/components/table';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HookView } from '$lib/ipc/bindings/HookView';
  import { cx } from '$lib/utils/cx';

  import { ACTION_LABELS, EVENT_ICONS, EVENT_LABELS, hookTitle, viewKey } from './hook-format';
  import { switchedOn, switchLock, teamSwitchedOn } from './hook-team';
  import HookStatusBadge from './HookStatusBadge.svelte';

  interface Props extends DocListHandlers<HookView> {
    /** The hooks of the page, in the order of the sort. */
    rows: HookView[];
    /** The pager under the table: the rows are one page of `total`. */
    pagination: { page: number; pageSize: number; total: number; onpage: (page: number) => void };
    /** The sort the hooks are in; null for the run order. */
    sort: Sort | null;
    onsort: (sort: Sort) => void;
    /** The hook whose panel is open. */
    openId: string | null;
    /** What a hook's Scope cell says. */
    scopeOf: (view: HookView) => DocScope;
    /** Hooks a change is being written for (by `viewKey`): their switch waits. */
    busy: ReadonlySet<string>;
  }

  let { rows, pagination, sort, onsort, openId, scopeOf, busy, onopen, onmove, ontoggle, table = $bindable() }: Props = $props();

  const COLUMNS = $derived([
    { key: 'hook', label: t('hooks.list.column.hook') },
    { key: 'scope', label: t('components.scope.label'), col: 'w-[13rem]' },
    { key: 'when', label: t('hooks.list.column.when'), col: 'w-[15rem]' },
    { key: 'does', label: t('hooks.list.column.does'), col: 'hidden w-[8.5rem] @min-[1000px]:table-column', head: 'hidden @min-[1000px]:table-cell' },
    { key: 'status', label: t('hooks.list.column.status'), col: 'w-[10.5rem]' },
    { key: 'on', label: t('hooks.list.column.on'), col: 'w-[5.5rem]', right: true, descFirst: true },
  ]);

  /** Why a row's switch is off, or null when it works: a hook that waits for approval or cannot run is never turned on by a click, as that would approve what the user has not seen. */
  function lockReason(view: HookView): string | null {
    const key = switchLock(view);

    return key === null ? null : t(key);
  }

  /** Whether the row's label offers to turn it off: a team hook as this computer has it, a local one as its file says. */
  function isOn(view: HookView): boolean {
    return view.origin === 'team' ? teamSwitchedOn(view) : (view.hook?.enabled ?? false);
  }
</script>

{#snippet cells(view: HookView)}
  {@const hook = view.hook}
  {@const key = viewKey(view)}
  {@const WhenIcon = hook ? EVENT_ICONS[hook.event] : null}

  <td role="gridcell" class="border-b border-line/60 px-3">
    <div class={cx('truncate font-medium', view.status === 'off' ? 'text-fg-2' : 'text-fg')} title={hookTitle(view)}>{hookTitle(view)}</div>
  </td>

  <DocScopeCell scope={scopeOf(view)} remote={view.origin === 'team'} />

  <td role="gridcell" class="border-b border-line/60 px-3 text-fg-2">
    {#if hook && WhenIcon}
      <div class="flex min-w-0 items-center gap-1.5">
        <WhenIcon size={ICON.sm} aria-hidden="true" class="shrink-0 text-fg-3" />
        <span class="truncate">{t(EVENT_LABELS[hook.event])}</span>
        {#if hook.matcher}<span class="h-5 max-w-[5rem] shrink-[4] truncate rounded-sm bg-surface-3 px-1.5 font-mono text-xs leading-5 text-fg-2" title={t('hooks.field.toolFilter')}>{hook.matcher}</span>{/if}
      </div>
    {:else}
      <span class="text-fg-3">–</span>
    {/if}
  </td>

  <td role="gridcell" class="hidden border-b border-line/60 px-3 text-fg-2 @min-[1000px]:table-cell">
    <div class="truncate">{#if hook}{t(ACTION_LABELS[hook.action.type])}{:else}–{/if}</div>
  </td>

  <td role="gridcell" class="border-b border-line/60 px-3"><HookStatusBadge status={view.status} /></td>

  <td role="gridcell" class="border-b border-line/60 px-3 text-right">
    <DocSwitchCell inRow label={t(isOn(view) ? 'hooks.switch.off' : 'hooks.switch.on', { name: hookTitle(view) })} checked={switchedOn(view)} lockReason={lockReason(view)} busy={busy.has(key)} onchange={(next) => ontoggle(view, next)} />
  </td>
{/snippet}

<DocTable bind:this={table} label={t('hooks.list.label')} columns={COLUMNS} {sort} {onsort} {rows} {pagination} rowKey={viewKey} {openId} {onopen} {onmove} {cells} />
