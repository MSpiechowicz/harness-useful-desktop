<script lang="ts">
  import Search from '@lucide/svelte/icons/search';

  import { t } from '$lib/i18n/i18n.svelte';
  import { palette } from '$lib/stores/palette.svelte';
  import { cx } from '$lib/utils/cx';

  import { ICON } from './icon';
  import Kbd from './Kbd.svelte';
  import Tooltip from './Tooltip.svelte';

  interface Props {
    /** Icon only, for the collapsed rail. */
    compact?: boolean;
  }

  let { compact = false }: Props = $props();
</script>

<!-- The rail's search field, under the logo as in the dashboard: a row-high button that opens the command palette. Its
     icon sits on the menu icons' column (same padding as a rail row), so collapsing the rail never moves it. -->
<Tooltip text={t('common.search')} shortcut="mod+k" side="right" disabled={!compact} class="flex w-full">
  {#snippet children({ describedby })}
    <button
      type="button"
      onclick={() => (palette.open = true)}
      aria-label={t('shell.search.aria')}
      aria-haspopup="dialog"
      aria-expanded={palette.open}
      aria-keyshortcuts="Control+K Meta+K"
      aria-describedby={describedby}
      class={cx(
        // The dashboard's search button in pixels (this app's root font is 14px): a 28px bordered field, 13px muted
        // text and small key caps. Collapsed, only the icon, centered.
        'flex h-[28px] w-full items-center gap-[10px] rounded-sm border border-line-strong bg-surface-2 text-[13px] font-normal whitespace-nowrap text-fg-3 shadow-raised transition-colors duration-[var(--dur-fast)] hover:bg-surface-3 hover:text-fg-2',
        compact ? 'justify-center px-0' : 'px-[12px]',
      )}
    >
      <Search size={ICON.sm} class="shrink-0" aria-hidden="true" />
      {#if !compact}
        <span class="min-w-0 flex-1 truncate text-left">{t('common.search')}</span>
        <span class="shrink-0" aria-hidden="true"><Kbd keys="mod+k" small /></span>
      {/if}
    </button>
  {/snippet}
</Tooltip>
