<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import type { Snippet } from 'svelte';
  import { slide } from 'svelte/transition';

  import { scaled } from '$lib/design/motion';
  import { cx } from '$lib/utils/cx';

  interface Props {
    open: boolean;
    ontoggle: () => void;
    /** Tints the icon tile: the agent thinking, a tool running, or the plain grey of the rest. */
    tone?: 'plain' | 'think' | 'run';
    /** A failed card draws its edge in red. */
    failed?: boolean;
    /** The 16px icon (or spinner) inside the 28px tile. */
    icon: Snippet;
    /** The title line. */
    title: Snippet;
    /** The status pill (a `StatusPill`); thinking has none. */
    status?: Snippet;
    /** Under the header while closed (a running tool's last output line). */
    peek?: Snippet;
    /** Between header and body, always shown (the subagent list). */
    extra?: Snippet;
    /** The body, shown while open. */
    body?: Snippet;
    /** Extra attributes for the outer card (`data-tool-status`). */
    [key: `data-${string}`]: string | undefined;
  }

  let { open, ontoggle, tone = 'plain', failed = false, icon, title, status, peek, extra, body, ...rest }: Props = $props();

  const TILES = { plain: 'bg-surface-2 text-fg-2', think: 'bg-think-wash text-think', run: 'bg-accent-wash text-accent-text' };
</script>

<!-- One shape for everything the agent does in a reply (thinking, tool calls): a 44px header with a 28px icon tile, the
     title, a fixed-width status pill so the status icons line up from row to row, and the chevron. Open, the body
     continues under the header, whose bottom corners go square so the hover wash leaves no dark wedges at the divider. -->
<div class={cx('border-card chat-radius bg-surface-1 bg-clip-padding transition-colors duration-[var(--dur-fast)]', failed ? 'border-danger-edge' : 'border-line hover:border-line-strong')} {...rest}>
  <button
    type="button"
    aria-expanded={open}
    class={cx('grid h-11 w-full grid-cols-[28px_minmax(0,1fr)_auto_16px] items-center gap-x-3 chat-radius pr-3.5 pl-2 text-left transition-colors hover:bg-hover active:bg-press', open && 'rounded-b-none')}
    onclick={ontoggle}
  >
    <span class={cx('grid size-7 place-items-center rounded-[8px]', TILES[tone])}>{@render icon()}</span>
    <span class="min-w-0 truncate text-sm">{@render title()}</span>
    <span>{@render status?.()}</span>
    <ChevronRight size={16} aria-hidden="true" class={cx('text-fg-3 transition-transform duration-[var(--dur-fast)]', open && 'rotate-90')} />
  </button>

  {@render peek?.()}
  {@render extra?.()}

  {#if open && body}
    <div transition:slide={{ duration: scaled(180) }} class="overflow-hidden border-t border-line">
      {@render body()}
    </div>
  {/if}
</div>
