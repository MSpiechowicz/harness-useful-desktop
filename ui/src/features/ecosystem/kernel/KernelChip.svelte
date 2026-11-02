<script lang="ts">
  import { FAMILY_NAMES } from '$lib/brand/family';
  import Logo from '$lib/components/Logo.svelte';
  import { cx } from '$lib/utils/cx';

  import KernelLed from './KernelLed.svelte';
  import type { Rect, Side } from './kernel-layout';
  import { moduleLabel } from './kernel-members';

  interface Props {
    chip: Rect;
    /** Pin centres along each side edge, as a y offset from the chip's top. */
    pins: Record<Side, number[]>;
    animated: boolean;
    /** Harness Desktop itself has an update: its LED pulses. Unknown or none: no LED. */
    updateAvailable: boolean;
    onhover?: (over: boolean) => void;
  }

  let { chip, pins, animated, updateAvailable, onhover }: Props = $props();

  // The chip is drawn on a 100 unit square and scaled, so the pins and the die stay in proportion at both sizes.
  const UNITS = 100;
  const PIN = { across: 4, out: 5, side: 6, sideAcross: 5 } as const;
  const EDGE_PINS = [18, 34, 50, 66, 82] as const;
  const MARK_SHARE = 0.34;
  const TRACES = 'M20 30 H34 L40 24 H52 M20 70 H30 L36 76 H50 M80 34 H68 L62 28 M80 66 H70 L64 72 H58 M24 50 H30 M70 50 H76';
  const VIAS = [[20, 30], [80, 34], [20, 70], [80, 66]] as const;

  const scale = $derived(chip.w / UNITS);
  const sidePins = $derived([...pins.left.map((y) => ({ side: 'left', y: y / scale })), ...pins.right.map((y) => ({ side: 'right', y: y / scale }))] as const);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class={cx('kernel-chip', animated && 'animated')}
  style:left="{chip.x}px"
  style:top="{chip.y}px"
  style:width="{chip.w}px"
  style:height="{chip.h}px"
  onpointerenter={() => onhover?.(true)}
  onpointerleave={() => onhover?.(false)}
>
  <svg viewBox="0 0 {UNITS} {UNITS}" width={chip.w} height={chip.h} aria-hidden="true">
    {#each EDGE_PINS as x (x)}
      <rect class="pin" x={x - PIN.across / 2} y={-PIN.out} width={PIN.across} height={PIN.out} rx="1" />
      <rect class="pin" x={x - PIN.across / 2} y={UNITS} width={PIN.across} height={PIN.out} rx="1" />
    {/each}
    {#each sidePins as pin, index (index)}
      <rect class="pin on" x={pin.side === 'left' ? -PIN.side : UNITS} y={pin.y - PIN.sideAcross / 2} width={PIN.side} height={PIN.sideAcross} rx="1" />
    {/each}

    <rect class="body" x="0" y="0" width={UNITS} height={UNITS} rx="14" />
    <rect class="die" x="12" y="12" width="76" height="76" rx="9" />
    <path class="circuit" d={TRACES} />
    {#each VIAS as [x, y] (`${x}-${y}`)}
      <circle class="circuit" cx={x} cy={y} r="1.6" />
    {/each}
  </svg>

  {#if animated}<span class="sweep"></span>{/if}

  <span class="mark"><Logo variant="mark" size={Math.round(chip.w * MARK_SHARE)} /></span>

  {#if updateAvailable}
    <span class="update" data-part="desktop-update" role="img" aria-label={moduleLabel({ name: FAMILY_NAMES.desktop, state: 'update' })}><KernelLed tone="update" pulse={animated} /></span>
  {/if}
</div>

<style>
  .kernel-chip {
    position: absolute;
    z-index: 3;
  }

  svg {
    position: absolute;
    inset: 0;
    overflow: visible;
  }

  .body {
    fill: var(--surface-2);
    stroke: var(--border-strong);
    stroke-width: 1;
  }

  .die {
    fill: var(--surface-1);
    stroke: var(--accent-2);
    stroke-opacity: 0.35;
    stroke-width: 1;
  }

  .circuit {
    fill: none;
    stroke: var(--border-strong);
    stroke-width: 1;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .pin {
    fill: var(--text-3);
    opacity: 0.55;
  }

  .pin.on {
    fill: var(--accent-2);
    opacity: 0.85;
  }

  .mark {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
  }

  /* A soft light crosses the die now and then, the chip at work. */
  .sweep {
    position: absolute;
    inset: 12%;
    border-radius: var(--kernel-card-radius);
    overflow: hidden;
    pointer-events: none;
  }

  .sweep::after {
    content: '';
    position: absolute;
    inset-block: 0;
    width: 50%;
    background: linear-gradient(90deg, transparent, color-mix(in srgb, var(--accent-2) 16%, transparent), transparent);
    animation: sweep 6s var(--ease-in-out) infinite;
  }

  @keyframes sweep {
    0% {
      transform: translateX(-120%);
    }
    55%,
    100% {
      transform: translateX(260%);
    }
  }

  /* The update LED sits in the chip's top right corner, outside the die. */
  .update {
    position: absolute;
    top: 7%;
    right: 7%;
  }
</style>
