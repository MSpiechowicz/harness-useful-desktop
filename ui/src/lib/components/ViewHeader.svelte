<script lang="ts">
  import type { Snippet } from 'svelte';
  import { fade } from 'svelte/transition';

  import { portal, SHELL_HEADER_SLOT } from '$lib/actions/portal';
  import { scaled } from '$lib/design/motion';
  import { cx } from '$lib/utils/cx';

  import { ICON, type IconComponent } from './icon';
  import PaletteButton from './PaletteButton.svelte';

  interface Props {
    /** What this view is: "Sessions". It is the page's one `h1`. */
    title: string;
    /** One short line of state after the title, e.g. "12 sessions". */
    context?: string;
    /** The view's icon, shown in a tile; ignored when `leading` is given. */
    icon?: IconComponent;
    /** Replaces the icon tile (a harness badge, for instance). */
    leading?: Snippet;
    /** Fills the space between the title and the actions, e.g. a tab strip. */
    center?: Snippet;
    /** The primary actions of the view, right-aligned. */
    actions?: Snippet;
    class?: string;
  }

  let { title, context, icon: Icon, leading, center, actions, class: className }: Props = $props();
</script>

<!-- Rendered into the shell's single top bar. The incoming header waits for the outgoing one to fade so the two
     never overlap; both play even though the view around them is what is being swapped (`|global`). -->
<header
  data-view-header
  use:portal={SHELL_HEADER_SLOT}
  in:fade|global={{ duration: scaled(160), delay: 80 }}
  out:fade|global={{ duration: scaled(100) }}
  class={cx('flex h-12 min-w-0 items-center gap-3 px-2', className)}
>
  {#if leading}
    {@render leading()}
  {:else if Icon}
    <span class="grid size-7 shrink-0 place-items-center rounded-sm border border-line bg-surface-2 text-accent-text">
      <Icon size={ICON.sm} strokeWidth={1.8} aria-hidden="true" />
    </span>
  {/if}

  <div class={cx('flex min-w-0 items-baseline gap-2', center ? 'shrink-0' : 'flex-1')}>
    <h1 class="truncate text-base leading-5 font-medium text-fg">{title}</h1>
    {#if context}
      <p class="truncate text-sm text-fg-3 max-[1000px]:hidden">{context}</p>
    {/if}
  </div>

  {#if center}
    <div class="flex min-w-0 flex-1 items-center">
      {@render center()}
    </div>
  {/if}

  <div class="flex shrink-0 items-center justify-end gap-2">
    <PaletteButton />
    {#if actions}
      {@render actions()}
    {/if}
  </div>
</header>
