<script lang="ts">
  import X from '@lucide/svelte/icons/x';
  import type { Snippet } from 'svelte';
  import { expoOut } from 'svelte/easing';
  import { fly } from 'svelte/transition';

  import { scaled, travel } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  import IconButton from './IconButton.svelte';
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
    onclose: () => void;
    children: Snippet;
  }

  let { open, title, subtitle = '', mono = false, inline = false, onclose, children }: Props = $props();

  const SLIDE_PX = 560;
</script>

{#snippet body()}
  <header class="flex shrink-0 items-start gap-2 border-b border-line py-3 pr-2 pl-5">
    <div class="min-w-0 flex-1">
      <h2 class={cx('truncate text-lg leading-6 font-semibold text-fg', mono && 'font-mono text-base')} {title}>{title}</h2>
      {#if subtitle}<p class="mt-0.5 truncate text-xs text-fg-3" title={subtitle}>{subtitle}</p>{/if}
    </div>

    <!-- The title line is 21px (1.5rem of a 14px root) and the button 36px: the pull of 8px above and below puts the glyph on the middle of the title line, whatever lies under it. -->
    <IconButton icon={X} label={t('components.panel.close')} class="-my-2 shrink-0" onclick={onclose} />
  </header>

  <div class="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-5 pt-4 pb-6">
    {@render children()}
  </div>
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
    class="absolute inset-y-0 right-0 z-20 flex flex-col overflow-hidden border-l border-line-strong bg-surface-1 shadow-pop"
  >
    {@render body()}
  </section>
{/if}
