<script lang="ts">
  import ChevronsLeft from '@lucide/svelte/icons/chevrons-left';
  import ChevronsRight from '@lucide/svelte/icons/chevrons-right';

  import { createSpring, moveSpring } from '$lib/design/motion';
  import IconButton from '$lib/components/IconButton.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { RAIL_EXPAND_MIN_WIDTH, ui } from '$lib/stores/ui-state.svelte';
  import { cx } from '$lib/utils/cx';

  import { railBadge } from './rail-badges';
  import RailItem from './RailItem.svelte';
  import { VIEWS, viewShortcut } from './views';

  let rail: HTMLElement | undefined = $state();
  let buttons: (HTMLButtonElement | undefined)[] = $state([]);
  let placed = false;

  const indicatorY = createSpring(0);
  const activeIndex = $derived(VIEWS.findIndex((view) => view.id === nav.current));
  const expanded = $derived(ui.railShowsLabels);
  const chat = $derived(agentSessions.active?.chat ?? null);

  function placeIndicator(): void {
    const button = buttons[activeIndex];
    if (!rail || !button) {
      return;
    }

    const y = button.getBoundingClientRect().top - rail.getBoundingClientRect().top;

    if (placed) {
      moveSpring(indicatorY, y);
    } else {
      indicatorY.set(y, { instant: true });
      placed = true;
    }
  }

  $effect(() => {
    void activeIndex;
    placeIndicator();
  });

  $effect(() => {
    if (!rail || typeof ResizeObserver === 'undefined') {
      return;
    }

    const observer = new ResizeObserver(() => {
      placed = false;
      placeIndicator();
    });
    observer.observe(rail);

    return () => observer.disconnect();
  });

  // Whether the window has room for labels; below that the rail stays icon-only whatever was chosen.
  $effect(() => {
    const query = window.matchMedia(`(min-width: ${RAIL_EXPAND_MIN_WIDTH}px)`);
    const update = (): void => void (ui.roomForLabels = query.matches);

    update();
    query.addEventListener('change', update);

    return () => query.removeEventListener('change', update);
  });
</script>

<nav
  bind:this={rail}
  aria-label="Primary"
  data-expanded={expanded}
  class={cx(
    'intro-enter intro-left relative flex shrink-0 flex-col gap-1 border-r border-line bg-surface-1/70 px-[12px] py-3 transition-[width] duration-[var(--dur-slow)] ease-[var(--ease-out-expo)]',
    expanded ? 'w-[var(--rail-expanded)]' : 'w-[var(--rail-collapsed)]',
  )}
>
  <!-- Active indicator: a glowing tile that springs between destinations. Pixel insets (not rem) keep it centered on
       the fixed-width rail at any root font size. -->
  <span
    aria-hidden="true"
    class="pointer-events-none absolute top-0 left-[12px] h-10 rounded-md border border-accent/30 bg-accent-wash shadow-glow transition-[width] duration-[var(--dur-slow)] ease-[var(--ease-out-expo)]"
    style:width="calc(100% - 24px)"
    style:transform="translateY({indicatorY.current}px)"
  ></span>

  {#each VIEWS as view, index (view.id)}
    <div class={cx('flex', view.id === 'settings' && 'mt-auto')}>
      <RailItem
        {view}
        shortcut={viewShortcut(view.id)}
        active={view.id === nav.current}
        {expanded}
        badge={railBadge(view.id, chat, git.status)}
        bind:button={buttons[index]}
        onselect={() => nav.go(view.id)}
      />
    </div>
  {/each}

  {#if ui.roomForLabels}
    <div class={cx('mt-1 flex border-t border-line pt-2', expanded ? 'justify-end' : 'justify-center')}>
      <IconButton
        icon={expanded ? ChevronsLeft : ChevronsRight}
        label={expanded ? 'Collapse sidebar' : 'Expand sidebar'}
        shortcut="mod+b"
        side="right"
        size="sm"
        aria-expanded={expanded}
        onclick={() => ui.toggleRail()}
      />
    </div>
  {/if}
</nav>
