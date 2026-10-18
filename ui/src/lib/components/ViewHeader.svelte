<script lang="ts">
  import type { Snippet } from 'svelte';
  import { fade } from 'svelte/transition';

  import { HEADER_CONTROLS_SLOT, HEADER_FIXED_SLOT, HEADER_TITLE_SLOT, portal } from '$lib/actions/portal';
  import { fixedOut, readyAfterMount, titleOut } from '$lib/design/header-motion';
  import { DURATION, scaled } from '$lib/design/motion';
  import { cx } from '$lib/utils/cx';

  import { ICON, type IconComponent } from './icon';

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
    /** The view-specific actions (search, a primary button, a switcher), right-aligned. They fade when the view changes. */
    actions?: Snippet;
    /**
     * What looks the same in every view and so must not look re-rendered when the view changes: the layout toggles and the
     * "⋯" menu, in that order, at the right end. It does not animate: the outgoing one (inert from the moment it leaves) stays until the
     * incoming one has its buttons, then the incoming one is in its place.
     */
    fixed?: Snippet;
    class?: string;
  }

  let { title, context, icon: Icon, leading, center, actions, fixed, class: className }: Props = $props();
</script>

<!-- Rendered into the shell's single top bar, in three places (see HEADER_TITLE_SLOT). When the view changes, the outgoing
     and incoming title sit in the same cell and crossfade in place (the outgoing one waits at full strength until the incoming
     one exists, so a view that loads late never leaves a dim bar). The controls fade out and then in, so two sets of buttons
     never show at once. The fixed end is swapped without a visible transition: the outgoing one is inert as soon as it leaves
     and the slot shows it until the incoming one has its buttons, and both look alike, so the toggles and the menu seem not to have
     moved. All of them play even though the view around them is what is being swapped (`|global`). The wrapper stays where the view put it while the three parts
     move to the top bar, so that removing the header never reaches past its own nodes: the portals take the parts away. -->
<div class="contents">
  <div
    data-view-header
    use:portal={HEADER_TITLE_SLOT}
    in:fade|global={{ duration: scaled(DURATION.base) }}
    out:titleOut|global
    class={cx('flex h-full min-w-0 items-center gap-3', className)}
  >
    {#if leading}
      {@render leading()}
    {:else if Icon}
      <span class="grid size-control shrink-0 place-items-center rounded-md border border-line bg-surface-2 text-accent-text">
        <Icon size={ICON.sm} strokeWidth={1.8} aria-hidden="true" />
      </span>
    {/if}

    <div class={cx('flex items-baseline gap-2', center ? 'shrink-0' : 'min-w-[var(--view-title-min)] flex-1')}>
      <h1 class="max-w-full shrink-0 truncate text-base leading-5 font-medium text-fg">{title}</h1>
      {#if context}
        <p class="min-w-0 truncate text-sm text-fg-3 max-[1000px]:hidden">{context}</p>
      {/if}
    </div>

    {#if center}
      <div class="flex min-w-0 flex-1 items-center">
        {@render center()}
      </div>
    {/if}
  </div>

  <!-- Only the search field gives way in a crowded header (it has its own floor): every other control keeps its size, so none can overlap another. -->
  <div
    use:portal={HEADER_CONTROLS_SLOT}
    in:fade|global={{ duration: scaled(DURATION.base), delay: scaled(DURATION.fast) }}
    out:fade|global={{ duration: scaled(DURATION.fast) }}
    class="flex h-full min-w-0 items-center justify-end gap-2 [&>*:not(.search-width)]:shrink-0 [&>.contents>*]:shrink-0"
  >
    {#if actions}
      {@render actions()}
    {/if}
  </div>

  <!-- Always present, even without content, so that a view with nothing here still replaces the previous view's. It only
       takes room (the gap before it) when it holds a button. -->
  <div use:portal={HEADER_FIXED_SLOT} use:readyAfterMount out:fixedOut|global class="flex h-full items-center gap-2 has-[button]:pl-2">
    {#if fixed}
      {@render fixed()}
    {/if}
  </div>
</div>
