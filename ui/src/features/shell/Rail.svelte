<script lang="ts">
  import PanelLeftClose from '@lucide/svelte/icons/panel-left-close';
  import PanelLeftOpen from '@lucide/svelte/icons/panel-left-open';

  import { createSpring, moveSpring } from '$lib/design/motion';
  import IconButton from '$lib/components/IconButton.svelte';
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

{#snippet toggle()}
  <IconButton
    icon={expanded ? PanelLeftClose : PanelLeftOpen}
    label={expanded ? 'Collapse sidebar' : 'Expand sidebar'}
    shortcut="mod+b"
    side="right"
    size="sm"
    aria-expanded={expanded}
    class="text-fg-2 hover:text-fg"
    onclick={() => ui.toggleRail()}
  />
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
  <!-- The brand sits top-left, on the same baseline as the top bar; the collapse toggle lives next to it. -->
  <div class={cx('flex h-11 shrink-0 items-center border-b border-line px-[12px]', expanded ? 'justify-between' : 'justify-center')}>
    <Logo size={expanded ? 15 : 18} wordmark={expanded} state={working ? 'working' : 'idle'} />

    {#if expanded && ui.roomForLabels}
      {@render toggle()}
    {/if}
  </div>

  <!-- Where you are: one control for the harness and the project. -->
  <div class={cx('flex shrink-0 border-b border-line px-[12px] py-2', !expanded && 'justify-center')}>
    <WorkspaceSwitcher compact={!expanded} />
  </div>

  {#if !expanded && ui.roomForLabels}
    <div class="flex justify-center pt-2">
      {@render toggle()}
    </div>
  {/if}

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
  </div>
</nav>
