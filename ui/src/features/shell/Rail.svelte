<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import Keyboard from '@lucide/svelte/icons/keyboard';
  import PanelLeftClose from '@lucide/svelte/icons/panel-left-close';
  import PanelLeftOpen from '@lucide/svelte/icons/panel-left-open';

  import { createSpring, moveSpring } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import { ICON, type IconComponent } from '$lib/components/icon';
  import Logo from '$lib/components/Logo.svelte';
  import PaletteButton from '$lib/components/PaletteButton.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { RAIL_EXPAND_MIN_WIDTH, ui } from '$lib/stores/ui-state.svelte';
  import { cx } from '$lib/utils/cx';

  import ChatTree from '../chat/ChatTree.svelte';

  import { railBadge } from './rail-badges';
  import RailFooter from './RailFooter.svelte';
  import RailItem from './RailItem.svelte';
  import { VIEWS, viewShortcut } from './views';

  interface Props {
    onshortcuts: () => void;
  }

  let { onshortcuts }: Props = $props();

  let rail: HTMLElement | undefined = $state();
  let buttons: (HTMLButtonElement | undefined)[] = $state([]);
  let placed = false;
  let list: HTMLElement | undefined = $state();
  /** Whether the open chats show under Chat; open unless the user folds them away. */
  let chatsOpen = $state(true);

  const indicatorY = createSpring(0);
  const activeIndex = $derived(VIEWS.findIndex((view) => view.id === nav.current));
  const expanded = $derived(ui.railShowsLabels);
  const chatCount = $derived(agentSessions.sessions.length + agentSessions.starting.length);
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
    // The chat cards push the rows below them down as chats open and close, so the indicator follows the list's height.
    if (list) {
      observer.observe(list);
    }

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

<!-- The last rows of the rail, under Settings: shaped like destinations, so they read as part of the list. -->
{#snippet action(label: string, Icon: IconComponent, shortcut: string, onclick: () => void, pressed?: boolean)}
  <Tooltip text={label} {shortcut} side="right" disabled={expanded} class="flex w-full">
    {#snippet children({ describedby })}
      <button
        type="button"
        aria-label={label}
        aria-expanded={pressed}
        aria-describedby={describedby}
        {onclick}
        class={cx('group/item flex h-[28px] w-full items-center gap-[10px] rounded-sm text-left text-fg-2 transition-[color,background-color,padding] duration-[var(--dur-fast)] hover:bg-hover hover:text-fg', expanded ? 'px-[12px]' : 'px-[15.5px]')}
      >
        <span class="grid size-[16px] shrink-0 place-items-center"><Icon size={ICON.sm} aria-hidden="true" /></span>
        {#if expanded}
          <span class="min-w-0 flex-1 truncate text-base whitespace-nowrap">{label}</span>
        {/if}
      </button>
    {/snippet}
  </Tooltip>
{/snippet}

<nav
  bind:this={rail}
  aria-label={t('shell.rail.label')}
  data-expanded={expanded}
  class={cx(
    'intro-enter intro-left relative flex shrink-0 flex-col border-r border-line bg-surface-1/70 transition-[width] duration-[var(--dur-slow)] ease-[var(--ease-out-expo)]',
    expanded ? 'w-[var(--rail-expanded)]' : 'w-[var(--rail-collapsed)]',
  )}
>
  <!-- The brand, as in the dashboard: a 56px row, 24px mark and 15px name, 16px from the edge. -->
  <div class={cx('flex h-[56px] shrink-0 items-center', expanded ? 'pl-[16px]' : 'justify-center')}>
    <Logo size={24} nameSize={15} wordmark={expanded} state={working ? 'working' : 'idle'} />
  </div>

  <!-- Search and commands right under the logo, as in the dashboard. -->
  <div class="flex shrink-0 px-[8px] pt-[4px]">
    <PaletteButton compact={!expanded} />
  </div>

  <!-- Active indicator: a tinted row that springs between destinations. Pixel insets (not rem) keep it centered on
       the fixed-width rail at any root font size. -->
  <span
    aria-hidden="true"
    class="pointer-events-none absolute top-0 left-[8px] h-[28px] rounded-sm bg-accent-wash transition-[width] duration-[var(--dur-slow)] ease-[var(--ease-out-expo)]"
    style:width="calc(100% - 16px)"
    style:transform="translateY({indicatorY.current}px)"
  ></span>

  <!-- 3px between rows (as between the chats under Chat), so neighbouring highlights never touch. -->
  <div bind:this={list} class="flex flex-col gap-[3px] px-[8px] py-[8px]">
    {#each VIEWS as view, index (view.id)}
      <!-- A group heading, or a hairline in its place while collapsed: one height for both, so rows never jump. -->
      {#if view.section}
        <div class="relative mt-[9px] mb-[1px] h-[16px] px-[12px]" aria-hidden="true">
          <span class={cx('block truncate text-[11px] leading-[16px] font-medium tracking-wide whitespace-nowrap text-fg-3 uppercase transition-opacity duration-[var(--dur-base)]', expanded ? 'opacity-100' : 'opacity-0')}>{view.section}</span>
          <span class={cx('absolute inset-x-[12px] top-1/2 h-px bg-line transition-opacity duration-[var(--dur-base)]', expanded ? 'opacity-0' : 'opacity-100')}></span>
        </div>
      {/if}

      <div class="relative flex">
        <RailItem
          {view}
          reserve={view.id === 'chat' && expanded && chatCount > 0}
          shortcut={viewShortcut(view.id)}
          active={view.id === nav.current}
          {expanded}
          badge={railBadge(view.id, chat, git.status)}
          bind:button={buttons[index]}
          onselect={() => nav.go(view.id)}
        />
        {#if view.id === 'chat' && expanded && chatCount > 0}
          <button
            type="button"
            aria-label={chatsOpen ? t('shell.rail.chats.hide') : t('shell.rail.chats.show')}
            aria-expanded={chatsOpen}
            onclick={() => (chatsOpen = !chatsOpen)}
            class={cx('absolute top-[2px] right-[6px] grid size-[24px] place-items-center rounded-sm transition-colors hover:bg-hover', nav.current === 'chat' ? 'text-accent-text' : 'text-fg-3 hover:text-fg')}
          >
            <ChevronRight size={ICON.sm} aria-hidden="true" class={cx('transition-transform duration-[var(--dur-base)]', chatsOpen && 'rotate-90')} />
          </button>
        {/if}
      </div>

      {#if view.id === 'chat' && expanded && chatCount > 0 && chatsOpen}
        <ChatTree />
      {/if}
    {/each}

    <div class="flex">{@render action(t('shell.shortcuts.title'), Keyboard, 'shift+?', onshortcuts)}</div>

    {#if ui.roomForLabels}
      <div class="flex">{@render action(expanded ? t('shell.rail.collapse') : t('shell.rail.expand'), expanded ? PanelLeftClose : PanelLeftOpen, 'mod+b', () => ui.toggleRail(), expanded)}</div>
    {/if}
  </div>

  <RailFooter {expanded} />
</nav>
