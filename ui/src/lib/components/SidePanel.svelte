<script lang="ts">
  import type { Snippet } from 'svelte';
  import { expoOut } from 'svelte/easing';
  import { fly } from 'svelte/transition';

  import { scaled, travel } from '$lib/design/motion';
  import { cx } from '$lib/utils/cx';

  import PaneHeader from './PaneHeader.svelte';
  import { SIDE_PANEL_WIDTH } from './side-panel';

  interface Props {
    open: boolean;
    /** Shown while the item is still being read; empty hides the header text but keeps the close button. */
    title: string;
    /** One line under the title: what the item is. */
    subtitle?: string;
    mono?: boolean;
    /** Fills the column it is placed in instead of sliding in over the right edge of its parent: no slide, no shadow. */
    inline?: boolean;
    /**
     * A panel that lays itself out: no shadow (the 1px panel edge alone sets it off the page it covers) and a body without
     * padding or gaps, so `top` and `footer` can run edge to edge and the content sets its own inset.
     */
    flat?: boolean;
    /** Fixed under the header, above the scrolling body (a status card, tabs). */
    top?: Snippet;
    /** Fixed under the scrolling body, edge to edge (a message box). */
    footer?: Snippet;
    onclose: () => void;
    children: Snippet;
  }

  let { open, title, subtitle = '', mono = false, inline = false, flat = false, top, footer, onclose, children }: Props = $props();

  const SLIDE_PX = 560;
</script>

<!-- A flat panel's subtitle is trimmed to its baseline, not to the descender line the header otherwise centres on, so the space
     under the line reads as equal to the space over the title (the glyphs' descenders hang free into it). The inner box truncates
     by itself: an ellipsis only works on the lines a box holds directly. -->
{#snippet subtitleLine()}{#if flat}<span class="block truncate py-[0.3em] -my-[0.3em] [text-box:trim-both_cap_alphabetic]">{subtitle}</span>{:else}{subtitle}{/if}{/snippet}

{#snippet body()}
  <PaneHeader {title} {mono} inset="panel" subtitle={subtitle ? subtitleLine : undefined} subtitleHint={subtitle} {onclose} />

  {@render top?.()}

  <div data-panel-body class={cx('flex min-h-0 flex-1 flex-col overflow-y-auto', flat ? 'panel-fade' : 'gap-4 panel-pad pt-4 pb-6')}>
    {@render children()}
  </div>

  {@render footer?.()}
{/snippet}

<!-- Slides in over the right side of its (relatively positioned) parent. The rest of the parent stays usable: the panel
     takes no scrim and no focus, so the table beside it keeps its arrow keys. Reduced motion fades it in place.
     Inline, it is a column of the view: it only fills its place, and what changes inside it is the view's business. -->
{#if inline}
  {#if open}
    <section aria-label={title} class="flex size-full min-h-0 flex-col overflow-hidden bg-surface-1">
      {@render body()}
    </section>
  {/if}
{:else if open}
  <section
    aria-label={title}
    in:fly={{ x: travel(SLIDE_PX), opacity: 0.2, duration: scaled(340), easing: expoOut }}
    out:fly={{ x: travel(SLIDE_PX), opacity: 0.2, duration: scaled(220), easing: expoOut }}
    style:width="min({SIDE_PANEL_WIDTH}, 100%)"
    class={cx('absolute inset-y-0 right-0 z-20 flex flex-col overflow-hidden border-l border-panel-edge bg-surface-1', !flat && 'shadow-pop')}
  >
    {@render body()}
  </section>
{/if}
