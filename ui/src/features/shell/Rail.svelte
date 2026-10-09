<script lang="ts">
  import Tooltip from '$lib/components/Tooltip.svelte';
  import { createSpring, moveSpring } from '$lib/design/motion';
  import { formatShortcut } from '$lib/utils/shortcuts';
  import { nav } from '$lib/stores/nav.svelte';
  import { cx } from '$lib/utils/cx';

  import { VIEWS, viewShortcut } from './views';

  let rail: HTMLElement | undefined = $state();
  let buttons: (HTMLButtonElement | undefined)[] = $state([]);
  let placed = false;

  const indicatorY = createSpring(0);
  const activeIndex = $derived(VIEWS.findIndex((view) => view.id === nav.current));

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
</script>

<nav bind:this={rail} aria-label="Primary" class="intro-enter intro-left relative flex w-16 shrink-0 flex-col items-center gap-1 border-r border-line bg-surface-1/70 py-3">
  <!-- Active indicator: a glowing tile that springs between destinations. -->
  <span
    aria-hidden="true"
    class="pointer-events-none absolute top-0 left-3 size-10 rounded-md border border-accent/30 bg-accent-wash shadow-glow"
    style:transform="translateY({indicatorY.current}px)"
  >
    <span class="absolute top-2 bottom-2 -left-3 w-[3px] rounded-r-full bg-[image:var(--brand-gradient)]"></span>
  </span>

  {#each VIEWS as view, index (view.id)}
    {@const active = view.id === nav.current}
    <div class={cx(view.id === 'settings' && 'mt-auto')}>
      <Tooltip text="{view.label} ({formatShortcut(viewShortcut(view.id)).join('+')})" side="right">
        {#snippet children({ describedby })}
          <button
            bind:this={buttons[index]}
            type="button"
            aria-label={view.label}
            aria-current={active ? 'page' : undefined}
            aria-describedby={describedby}
            onclick={() => nav.go(view.id)}
            class={cx(
              'relative grid size-10 place-items-center rounded-md transition-[color,transform] duration-[var(--dur-fast)] active:scale-90',
              active ? 'text-accent-text' : 'text-fg-2 hover:bg-hover hover:text-fg',
            )}
          >
            <view.icon size={20} strokeWidth={1.8} aria-hidden="true" />
          </button>
        {/snippet}
      </Tooltip>
    </div>
  {/each}
</nav>
