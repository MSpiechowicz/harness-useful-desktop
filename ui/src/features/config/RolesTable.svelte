<script lang="ts">
  import HighlightedText from '$lib/components/HighlightedText.svelte';
  import { matchIndices } from '$lib/components/config-tree';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  import { meterBars, METER_SIZE, type RoleRow } from './structured';

  interface Props {
    rows: RoleRow[];
    /** Accessible name of the table. */
    label: string;
    /** Normalized search needle; empty highlights nothing. */
    needle?: string;
  }

  let { rows, label, needle = '' }: Props = $props();

  const BAR_HEIGHTS = ['h-[4px]', 'h-[7px]', 'h-[10px]', 'h-[12px]'];
  const HEAD_CLASS = 'h-[24px] text-left text-[11px] leading-[16px] font-medium tracking-wide text-fg-3 uppercase';
</script>

<table aria-label={label} class="w-full table-fixed text-sm">
  <thead>
    <tr>
      <th scope="col" class={cx(HEAD_CLASS, 'w-[140px]')}>{t('components.configView.roles.role')}</th>
      <th scope="col" class={HEAD_CLASS}>{t('components.configView.roles.model')}</th>
      <th scope="col" class={cx(HEAD_CLASS, 'w-[120px]')}>{t('components.configView.roles.thinking')}</th>
    </tr>
  </thead>
  <tbody>
    {#each rows as row (row.role)}
      <tr class="border-t border-line">
        <th scope="row" class="h-[34px] truncate pr-3 text-left font-normal text-fg-2">
          <HighlightedText text={row.role} indices={matchIndices(row.role, needle)} />
        </th>
        <td class="truncate pr-3 font-mono text-[12.5px]">
          <span class="text-fg-3"><HighlightedText text={`${row.ref.provider}/`} indices={matchIndices(`${row.ref.provider}/`, needle)} /></span><span class="text-success"
            ><HighlightedText text={row.ref.model} indices={matchIndices(row.ref.model, needle)} /></span
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
      </tr>
    {/each}
  </tbody>
</table>
