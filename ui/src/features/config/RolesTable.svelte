<script lang="ts">
  import HighlightedText from '$lib/components/HighlightedText.svelte';
  import { matchIndices } from '$lib/components/config-tree';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  import RolePicker from './RolePicker.svelte';
  import type { RoleEditing } from './role-edit';
  import { meterBars, METER_SIZE, type RoleRow } from './structured';

  interface Props {
    rows: RoleRow[];
    /** What the card offers to do with its roles; absent when it is read-only. */
    editing?: RoleEditing;
    /** Accessible name of the table. */
    label: string;
    /** Normalized search needle; empty highlights nothing. */
    needle?: string;
  }

  let { rows, editing, label, needle = '' }: Props = $props();

  const BAR_HEIGHTS = ['h-[4px]', 'h-[7px]', 'h-[10px]', 'h-[12px]'];
  // The models the roles already use, offered when mapping or changing one.
  const inUse = $derived([...new Set(rows.map((row) => row.text))]);
  // Change and Map share one right-aligned actions column.
  const actions = $derived(Boolean(editing?.change || editing?.map));
  // One height for every row, a mapped one, one with a Change button and one with a Map button, so none of them moves the table.
  // The rule above a row is a shadow, not a border: a border would add to the height of the rows that hold a button.
  const ROW_CLASS = '[&>*]:shadow-[inset_0_1px_0_0_var(--color-line)]';
  const ROLE_CLASS = 'h-control truncate pr-3 text-left font-normal text-fg-2';
  const HEAD_CLASS = 'h-[24px] text-left text-[11px] leading-[16px] font-medium tracking-wide text-fg-3 uppercase';
</script>

<table aria-label={label} class="w-full table-fixed text-sm">
  <thead>
    <tr>
      <th scope="col" class={cx(HEAD_CLASS, 'w-[140px]')}>{t('components.configView.roles.role')}</th>
      <th scope="col" class={HEAD_CLASS}>{t('components.configView.roles.model')}</th>
      <th scope="col" class={cx(HEAD_CLASS, 'w-[120px]')}>{t('components.configView.roles.thinking')}</th>
      {#if actions}
        <th scope="col" class={cx(HEAD_CLASS, 'w-[88px]')}><span class="sr-only">{t('components.configView.roles.actions')}</span></th>
      {/if}
    </tr>
  </thead>
  <tbody>
    {#each rows as row (row.role)}
      <tr class={ROW_CLASS}>
        <th scope="row" class={ROLE_CLASS}>
          <HighlightedText text={row.role} indices={matchIndices(row.role, needle)} />
        </th>
        <td class="truncate pr-3 font-mono text-[12.5px]">
          <!-- A Claude Code selector names no provider. -->
          {#if row.ref.provider !== null}<span class="text-fg-3"><HighlightedText text={`${row.ref.provider}/`} indices={matchIndices(`${row.ref.provider}/`, needle)} /></span>{/if}<span
            class="text-success"><HighlightedText text={row.ref.model} indices={matchIndices(row.ref.model, needle)} /></span
          >
        </td>
        <td>
          {#if row.ref.level === null}
            <span class="text-fg-3">—</span>
          {:else}
            <span class="inline-flex items-center gap-2 text-xs text-fg-2">
              <span class="inline-flex h-[12px] items-end gap-[2px]" aria-hidden="true">
                {#each { length: METER_SIZE } as _, bar (bar)}
                  <i class={cx('w-[3px] rounded-[1px]', BAR_HEIGHTS[bar], bar < meterBars(row.ref.level) ? 'bg-accent' : 'bg-surface-4')}></i>
                {/each}
              </span>
              <HighlightedText text={row.ref.level} indices={matchIndices(row.ref.level, needle)} />
            </span>
          {/if}
        </td>
        {#if actions}
          <td>
            {#if editing?.change}
              <div class="flex justify-end">
                <RolePicker kind="change" role={row.role} rules={editing.rules} current={row.text} suggestions={inUse} onpick={editing.change} />
              </div>
            {/if}
          </td>
        {/if}
      </tr>
    {/each}
    {#each editing?.unmapped ?? [] as item (item.role)}
      <tr class={ROW_CLASS}>
        <th scope="row" class={ROLE_CLASS}>
          <HighlightedText text={item.role} indices={matchIndices(item.role, needle)} />
        </th>
        <td class="truncate pr-3 text-sm text-fg-3">
          {item.inheritsSessionModel ? t('components.configView.roles.unmappedSession') : t('components.configView.roles.unmappedDefault')}
        </td>
        <td></td>
        {#if actions}
          <td>
            {#if editing?.map}
              <div class="flex justify-end">
                <RolePicker kind="map" role={item.role} rules={editing.rules} suggestions={inUse} onpick={editing.map} />
              </div>
            {/if}
          </td>
        {/if}
      </tr>
    {/each}
  </tbody>
</table>
