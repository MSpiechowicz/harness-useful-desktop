<script lang="ts">
  import ArrowUp from '@lucide/svelte/icons/arrow-up';
  import Check from '@lucide/svelte/icons/check';
  import Plus from '@lucide/svelte/icons/plus';

  import { FAMILY } from '$lib/brand/family';
  import Logo from '$lib/components/Logo.svelte';
  import { rise } from '$lib/design/motion';
  import type { EcosystemItem } from '$lib/ipc/bindings/EcosystemItem';
  import { motion } from '$lib/stores/motion.svelte';
  import { cx } from '$lib/utils/cx';

  import { installedSummary, itemState, SHORT_NAMES } from './ecosystem-items';

  interface Props {
    items: EcosystemItem[];
  }

  let { items }: Props = $props();

  // A family tree in a fixed 240 x 178 px box that the SVG maps 1:1: the app on top, the companions in a row below,
  // joined by one trunk and a bus that meet in a T. The box is trimmed to the drawing (the outer circles touch its
  // sides; the wider "Dashboard" label may hang into the gutter), so the card's padding frames it evenly. Every line
  // is horizontal or vertical, 2 px wide on whole pixels, so it renders crisp in WebKitGTK; only the elbows are rounded.
  const WIDTH = 240;
  const HEIGHT = 178;
  const HUB = { x: 120, y: 28, size: 56 } as const;
  const NODE = 44;
  const ROW_Y = 132;
  const COLUMNS = [24, 120, 216] as const;
  const BUS_Y = 84;
  const HALF_STROKE = 1;
  const ELBOW = 10;

  const hubBottom = HUB.y + HUB.size / 2;
  const nodeTop = ROW_Y - NODE / 2;

  /** From the edge of the trunk, along the bus, down into the companion's circle; the middle one continues the trunk. */
  function branch(x: number): string {
    if (x === HUB.x) {
      return `M ${x} ${BUS_Y + HALF_STROKE} V ${nodeTop}`;
    }

    const side = Math.sign(x - HUB.x);

    return [
      `M ${HUB.x + side * HALF_STROKE} ${BUS_Y}`,
      `H ${x - side * ELBOW}`,
      `Q ${x} ${BUS_Y} ${x} ${BUS_Y + ELBOW}`,
      `V ${nodeTop}`,
    ].join(' ');
  }

  const shown = $derived(items.slice(0, COLUMNS.length));
  const animated = $derived(motion.level === 'full');
  const layers = $derived(shown.map((item, index) => ({ item, index, installed: itemState(item) === 'installed' })));

  const description = $derived(
    `${installedSummary(items)}: ${items.map((item) => `${item.name} ${itemState(item) !== 'installed' ? 'not installed' : item.updateAvailable ? 'installed, update available' : 'installed'}`).join(', ')}`,
  );
</script>

<div role="img" aria-label={description} class="relative mx-auto shrink-0" style:width="{WIDTH}px" style:height="{HEIGHT}px">
  <svg width={WIDTH} height={HEIGHT} viewBox="0 0 {WIDTH} {HEIGHT}" class="absolute inset-0 overflow-visible" aria-hidden="true">
    <defs>
      <radialGradient id="family-glow">
        <stop offset="0%" stop-color="var(--accent-2)" stop-opacity="0.18" />
        <stop offset="100%" stop-color="var(--accent-2)" stop-opacity="0" />
      </radialGradient>
    </defs>

    <circle cx={HUB.x} cy={HUB.y} r="64" fill="url(#family-glow)" />
    <path d="M {HUB.x} {hubBottom} V {BUS_Y + HALF_STROKE}" stroke="var(--accent-2)" stroke-width="2" fill="none" />

    {#each layers as { item, index, installed } (item.id)}
      {@const path = branch(COLUMNS[index])}
      {#if installed}
        <path d={path} stroke={FAMILY[item.id][1]} stroke-width="2" fill="none" />
        <!-- A brighter stretch runs up the branch into the app, inside the same 2 px line: data flowing in. -->
        {#if animated}
          <path d={path} pathLength="100" stroke="white" stroke-opacity="0.75" stroke-width="2" fill="none" class="family-pulse" style:animation-delay="{index * 0.9}s" />
        {/if}
      {:else}
        <path d={path} stroke="var(--text-3)" stroke-opacity="0.6" stroke-width="2" stroke-dasharray="4 4" fill="none" />
      {/if}
    {/each}
  </svg>

  <span
    class="absolute grid place-items-center rounded-full border border-line-strong bg-surface-2 shadow-float"
    style:left="{HUB.x - HUB.size / 2}px"
    style:top="{HUB.y - HUB.size / 2}px"
    style:width="{HUB.size}px"
    style:height="{HUB.size}px"
  >
    <Logo variant="mark" size={26} />
  </span>

  {#each shown as item, index (item.id)}
    {@const installed = itemState(item) === 'installed'}
    {@const updating = installed && item.updateAvailable}

    <!-- Only the circle sits on the grid; the name hangs below it. -->
    <div
      in:rise={{ y: 10, delay: 120 + index * 90, duration: 480 }}
      class="absolute"
      style:left="{COLUMNS[index] - NODE / 2}px"
      style:top="{nodeTop}px"
      style:width="{NODE}px"
      style:height="{NODE}px"
    >
      <span class={cx('relative grid size-full place-items-center rounded-full border bg-surface-1 shadow-raised', installed ? 'border-line-strong' : 'border-dashed border-line-strong')}>
        <!-- Each companion wears its own family mark; one not installed yet is shown greyed out. -->
        <span class={cx('grid place-items-center', !installed && 'opacity-45 grayscale')} aria-hidden="true">
          <Logo variant="mark" product={item.id} size={18} />
        </span>
        <span
          class={cx(
            'absolute -top-0.5 -right-0.5 grid size-4 place-items-center rounded-full ring-2 ring-surface-1',
            updating ? 'bg-accent-fill text-white' : installed ? 'bg-success text-surface-1' : 'bg-surface-4 text-fg-2',
          )}
        >
          {#if updating}<ArrowUp size={11} strokeWidth={3.5} aria-hidden="true" />{:else if installed}<Check size={11} strokeWidth={3.5} aria-hidden="true" />{:else}<Plus size={11} strokeWidth={3.5} aria-hidden="true" />{/if}
        </span>
      </span>
      <span class={cx('absolute top-full -right-10 -left-10 mt-2 text-center text-xs font-medium whitespace-nowrap', installed ? 'text-fg-2' : 'text-fg-3')}>{SHORT_NAMES[item.id]}</span>
    </div>
  {/each}
</div>

<style>
  .family-pulse {
    stroke-dasharray: 18 182;
    stroke-dashoffset: -100;
    animation: family-flow 3.2s cubic-bezier(0.45, 0, 0.25, 1) infinite;
  }

  /* Each path runs from the app down to the companion, so the dash travels backwards: from the companion up. */
  @keyframes family-flow {
    0% {
      stroke-dashoffset: -100;
      opacity: 0;
    }
    12% {
      opacity: 1;
    }
    50% {
      stroke-dashoffset: 18;
      opacity: 0;
    }
    100% {
      stroke-dashoffset: 18;
      opacity: 0;
    }
  }
</style>
