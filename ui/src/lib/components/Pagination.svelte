<script lang="ts">
  import ChevronLeft from '@lucide/svelte/icons/chevron-left';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';

  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';
  import { formatCount } from '$lib/utils/format';
  import { pageCount, pageItems } from '$lib/utils/pagination';

  import Button from './Button.svelte';
  import { ICON } from './icon';

  interface Props {
    /** The zero-based page on screen. */
    page: number;
    pageSize: number;
    /** How many entries there are in all, not just on this page. */
    total: number;
    onpage: (page: number) => void;
  }

  let { page, pageSize, total, onpage }: Props = $props();

  const pages = $derived(pageCount(total, pageSize));
  const from = $derived(page * pageSize + 1);
  const to = $derived(Math.min(total, (page + 1) * pageSize));
</script>

<!-- "1–50 of 1,234" on the left, the pages on the right. A list that fits one page shows only the range. -->
<nav aria-label={t('components.pagination.label')} class="flex h-14 shrink-0 items-center justify-between gap-3 border-t border-line px-4">
  <p class="tabular text-sm text-fg-3" role="status">{t('components.pagination.range', { from: formatCount(from), to: formatCount(to), total: formatCount(total) })}</p>

  {#if pages > 1}
    <div class="flex items-center gap-1">
      <Button variant="ghost" disabled={page === 0} onclick={() => onpage(page - 1)}>
        {#snippet leading()}<ChevronLeft size={ICON.sm} aria-hidden="true" />{/snippet}
        {t('components.pagination.previous')}
      </Button>

      <ul class="flex items-center gap-1 max-[700px]:hidden">
        {#each pageItems(page, pages) as item, index (item === 'gap' ? `gap-${index}` : item)}
          <li>
            {#if item === 'gap'}
              <span class="grid size-control place-items-center text-fg-3" aria-hidden="true">…</span>
            {:else}
              <button
                type="button"
                aria-label={t('components.pagination.page', { page: formatCount(item + 1) })}
                aria-current={item === page ? 'page' : undefined}
                class={cx(
                  'tabular grid size-control min-w-control place-items-center rounded-md px-2 text-sm transition-colors duration-[var(--dur-fast)]',
                  item === page ? 'bg-accent-wash font-medium text-accent-text' : 'text-fg-2 hover:bg-hover hover:text-fg',
                )}
                onclick={() => onpage(item)}
              >
                {formatCount(item + 1)}
              </button>
            {/if}
          </li>
        {/each}
      </ul>

      <Button variant="ghost" disabled={page >= pages - 1} onclick={() => onpage(page + 1)}>
        {t('components.pagination.next')}
        {#snippet trailing()}<ChevronRight size={ICON.sm} aria-hidden="true" />{/snippet}
      </Button>
    </div>
  {/if}
</nav>
