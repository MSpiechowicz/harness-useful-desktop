<script lang="ts">
  import type { Snippet } from 'svelte';

  import { ICON, type IconComponent } from './icon';
  import { cx } from '$lib/utils/cx';

  type Width = 'full' | 'wide' | 'reading' | 'narrow';

  interface Props {
    /** What this view is: "Sessions". It is the page's one `h1`. */
    title: string;
    /** One line of scope or state under the title, e.g. "OMP · demo · 12 sessions". */
    context?: string;
    /** The view's icon, shown in a tile; ignored when `leading` is given. */
    icon?: IconComponent;
    /** Replaces the icon tile (a harness badge, for instance). */
    leading?: Snippet;
    /** Fills the space between the title and the actions, e.g. a tab strip. */
    center?: Snippet;
    /** The primary actions of the view, right-aligned. */
    actions?: Snippet;
    /** A toolbar or tab strip that belongs to the header and shares its background. */
    below?: Snippet;
    /** Aligns the header content with a page container (`full` spans the window). */
    width?: Width;
    class?: string;
  }

  let { title, context, icon: Icon, leading, center, actions, below, width = 'full', class: className }: Props = $props();

  const WIDTHS: Record<Width, string> = {
    full: 'px-[var(--gutter)]',
    wide: 'page-wide',
    reading: 'page-reading',
    narrow: 'page-narrow',
  };
</script>

<header data-view-header class={cx('sticky top-0 z-20 shrink-0 border-b border-line bg-surface-1/80 backdrop-blur-xl', className)}>
  <div class={cx('flex min-h-14 items-center gap-3 py-2', WIDTHS[width])}>
    {#if leading}
      {@render leading()}
    {:else if Icon}
      <span class="grid size-9 shrink-0 place-items-center rounded-md border border-line bg-surface-2 text-accent-text shadow-raised">
        <Icon size={ICON.md} strokeWidth={1.8} aria-hidden="true" />
      </span>
    {/if}

    <div class={cx('flex min-w-0 flex-col', center ? 'shrink-0' : 'flex-1')}>
      <h1 class="truncate text-lg leading-6 text-fg">{title}</h1>
      {#if context}
        <p class="truncate text-sm text-fg-2">{context}</p>
      {/if}
    </div>

    {#if center}
      <div class="flex min-w-0 flex-1 items-center">
        {@render center()}
      </div>
    {/if}

    {#if actions}
      <div class="flex shrink-0 flex-wrap items-center justify-end gap-2">
        {@render actions()}
      </div>
    {/if}
  </div>

  {#if below}
    {@render below()}
  {/if}
</header>
