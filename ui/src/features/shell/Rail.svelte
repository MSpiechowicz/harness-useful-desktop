<script lang="ts">
  import PanelLeftClose from '@lucide/svelte/icons/panel-left-close';
  import PanelLeftOpen from '@lucide/svelte/icons/panel-left-open';

  import { createSpring, moveSpring } from '$lib/design/motion';
  import Kbd from '$lib/components/Kbd.svelte';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import { ICON } from '$lib/components/icon';
  import Logo from '$lib/components/Logo.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { RAIL_EXPAND_MIN_WIDTH, ui } from '$lib/stores/ui-state.svelte';
  import { cx } from '$lib/utils/cx';

  import { railBadge } from './rail-badges';
  import RailItem from './RailItem.svelte';
  import { VIEWS, viewShortcut } from './views';
  import WorkspaceSwitcher from './WorkspaceSwitcher.svelte';

  let rail: HTMLElement | undefined = $state();
  let buttons: (HTMLButtonElement | undefined)[] = $state([]);
  let placed = false;

  const indicatorY = createSpring(0);
  const activeIndex = $derived(VIEWS.findIndex((view) => view.id === nav.current));
  const expanded = $derived(ui.railShowsLabels);
  const chat = $derived(agentSessions.active?.chat ?? null);
  // The mark blinks while any agent, not just the visible one, is working.
  const working = $derived(agentSessions.sessions.some((session) => session.chat.phase === 'streaming'));

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

<!-- The last row of the rail, under Settings: shaped like a destination, so it reads as part of the list. -->
{#snippet toggle()}
  {@const label = expanded ? 'Collapse sidebar' : 'Expand sidebar'}
  {@const Icon = expanded ? PanelLeftClose : PanelLeftOpen}
  <Tooltip text={label} shortcut="mod+b" side="right" disabled={expanded} class="flex w-full">
    {#snippet children({ describedby })}
      <button
        type="button"
        aria-label={label}
        aria-expanded={expanded}
        aria-describedby={describedby}
        onclick={() => ui.toggleRail()}
        class="group/item flex h-10 w-full items-center gap-3 rounded-md px-[9.5px] text-left text-fg-2 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg"
      >
        <span class="grid size-[20px] shrink-0 place-items-center"><Icon size={ICON.lg} strokeWidth={1.8} aria-hidden="true" /></span>
        {#if expanded}
          <span class="min-w-0 flex-1 truncate text-base font-medium whitespace-nowrap">{label}</span>
          <span class="hidden shrink-0 group-hover/item:inline-flex group-focus-visible/item:inline-flex"><Kbd keys="mod+b" /></span>
        {/if}
      </button>
    {/snippet}
  </Tooltip>
{/snippet}

<nav
  bind:this={rail}
  aria-label="Primary"
  data-expanded={expanded}
  class={cx(
    'intro-enter intro-left relative flex shrink-0 flex-col border-r border-line bg-surface-1/70 transition-[width] duration-[var(--dur-slow)] ease-[var(--ease-out-expo)]',
    expanded ? 'w-[var(--rail-expanded)]' : 'w-[var(--rail-collapsed)]',
  )}
>
  <!-- The brand sits top-left, on the same baseline as the top bar. Expanded, the mark is centered on the menu icons'
       column (12px gutter + 9.5px item padding + half of the 20px icon, less half of the ~17px mark). -->
  <div class={cx('flex h-12 shrink-0 items-center border-b border-line', expanded ? 'pl-[23px]' : 'justify-center px-[12px]')}>
    <Logo size={expanded ? 15 : 18} wordmark={expanded} state={working ? 'working' : 'idle'} />
  </div>

  <!-- Where you are: one control for the harness and the project. -->
  <div class={cx('flex shrink-0 border-b border-line px-[12px] py-2', !expanded && 'justify-center')}>
    <WorkspaceSwitcher compact={!expanded} />
  </div>

  <!-- Active indicator: a glowing tile that springs between destinations. Pixel insets (not rem) keep it centered on
       the fixed-width rail at any root font size. -->
  <span
    aria-hidden="true"
    class="pointer-events-none absolute top-0 left-[12px] h-10 rounded-md border border-accent/30 bg-accent-wash shadow-glow transition-[width] duration-[var(--dur-slow)] ease-[var(--ease-out-expo)]"
    style:width="calc(100% - 24px)"
    style:transform="translateY({indicatorY.current}px)"
  ></span>

  <div class="flex flex-col gap-1 px-[12px] py-2">
    {#each VIEWS as view, index (view.id)}
      {#if view.id === 'settings'}
        <div class="my-1 border-t border-line" aria-hidden="true"></div>
      {/if}

      <div class="flex">
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
      <div class="flex">{@render toggle()}</div>
    {/if}
  </div>
</nav>
