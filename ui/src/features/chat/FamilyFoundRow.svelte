<script lang="ts">
  import ExternalLink from '@lucide/svelte/icons/external-link';
  import Eye from '@lucide/svelte/icons/eye';
  import Play from '@lucide/svelte/icons/play';
  import ThumbsUp from '@lucide/svelte/icons/thumbs-up';
  import type { Snippet } from 'svelte';

  import Tooltip from '$lib/components/Tooltip.svelte';
  import type { RowMarks } from '$lib/chat/use-index';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  interface Props extends RowMarks {
    /** The badge of the row (its kind, its format); none when nothing is known of the row. */
    badge?: Snippet;
    title: string;
    /** The line under the title; none while nothing is known to say. */
    meta?: string;
    /** The name of the button that opens the row in its own view, and what it does. */
    showLabel: string;
    onshow: () => void;
  }

  let { badge, title, meta = '', use, dim = false, showLabel, onshow }: Props = $props();
</script>

<!-- One row of a Harness family card, whatever it lists: a badge, the title with what is known of it, what the reply did with it, and the way to open it. -->
<li class="grid grid-cols-[108px_minmax(0,1fr)_auto] items-center gap-x-3 px-[18px] py-2.5 transition-colors duration-[var(--dur-fast)] hover:bg-hover">
  <span class="justify-self-start">{@render badge?.()}</span>

  <div class="flex min-w-0 flex-col gap-0.5">
    <span class={cx('truncate text-sm', dim ? 'text-fg-2' : 'text-fg')}>{title}</span>
    {#if meta}<span class="truncate text-xs text-fg-3" title={meta}>{meta}</span>{/if}
    {#if use?.kind === 'useful' && use.reason}
      <span class="line-clamp-2 text-[12.5px] leading-snug text-fg-2" title={use.reason}>{t('chat.family.row.reason', { reason: use.reason })}</span>
    {/if}
  </div>

  <div class="flex items-center gap-2">
    {#if use?.kind === 'useful'}
      <span class="inline-flex items-center gap-1.5 text-xs font-medium text-info"><ThumbsUp size={14} aria-hidden="true" />{t('chat.family.row.useful')}</span>
    {:else if use?.kind === 'opened'}
      <span class="inline-flex items-center gap-1.5 text-xs text-fg-3"><Eye size={14} aria-hidden="true" />{t('chat.family.row.opened')}</span>
    {:else if use?.kind === 'ran'}
      <span class="inline-flex items-center gap-1.5 text-xs text-fg-3"><Play size={14} aria-hidden="true" />{t('chat.family.row.ran')}</span>
    {/if}

    <Tooltip text={showLabel}>
      {#snippet children({ describedby })}
        <button
          type="button"
          aria-label={showLabel}
          aria-describedby={describedby}
          class="inline-grid size-7 place-items-center rounded-sm text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg active:bg-press"
          onclick={onshow}
        >
          <ExternalLink size={16} aria-hidden="true" />
        </button>
      {/snippet}
    </Tooltip>
  </div>
</li>
