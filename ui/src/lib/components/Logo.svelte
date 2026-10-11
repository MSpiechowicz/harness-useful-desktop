<script lang="ts">
  import { FAMILY, FAMILY_NAMES, type FamilyProduct } from '$lib/brand/family';
  import { motion } from '$lib/stores/motion.svelte';
  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  interface Props {
    /** Height of the mark in px; the name scales with it unless `nameSize` is given. */
    size?: number;
    /** Show the name next to the mark. */
    wordmark?: boolean;
    /** `lockup` mark + name, `mark` the glyph alone cropped flush. */
    variant?: 'lockup' | 'mark';
    /** Idle is completely still; working blinks the cursor. */
    state?: 'idle' | 'working';
    /** Font size of the name in px. */
    nameSize?: number;
    /** Which family product's gradient the mark wears; Harness Desktop by default. */
    product?: FamilyProduct;
    class?: string;
  }

  let {
    size = 24,
    wordmark = false,
    variant = wordmark ? 'lockup' : 'mark',
    // Renamed locally: a binding called `state` would shadow the `$state` rune.
    state: activity = 'idle',
    nameSize = Math.round(size * 0.93 * 2) / 2,
    product = 'desktop',
    class: className,
  }: Props = $props();

  /*
   * The shared Harness mark (../harness-useful-logo, renders/desktop/mark.svg), tipped pose: the H leans 7 degrees
   * toward its fallen lower-right leg, which lies on the baseline as the cursor. Same paths and gradient as the renders.
   */
  const H = 'M18 20 H33 V50 H58 V20 H73 V64 H33 V94 H18 Z';
  const CURSOR = 'M73 79 H103 V94 H73 Z';
  const BOX = { x: 16.112, y: 16.724, width: 88.888, height: 79.276 };

  const gradient = uid('logo-gradient');
  const stops = $derived(FAMILY[product]);

  const blinking = $derived(activity === 'working' && motion.level === 'full');
  const width = $derived((size * BOX.width) / BOX.height);
  const showName = $derived(wordmark || variant === 'lockup');
</script>

<span class={cx('logo inline-flex items-center', className)} style:font-size="{nameSize}px" role="img" aria-label={FAMILY_NAMES[product]} data-state={activity}>
  <svg {width} height={size} viewBox="{BOX.x} {BOX.y} {BOX.width} {BOX.height}" aria-hidden="true">
    <defs>
      <!-- Defined inside each SVG: a gradient shared across SVGs breaks when the first instance unmounts. -->
      <linearGradient id={gradient} gradientUnits="userSpaceOnUse" x1="0" y1="94" x2="0" y2="20">
        <stop offset="0" stop-color={stops[0]} />
        <stop offset="1" stop-color={stops[1]} />
      </linearGradient>
    </defs>

    <g fill="url(#{gradient})">
      <path d={H} transform="rotate(7 33 94)" />
      <path class={['cursor', blinking && 'blink']} data-part="cursor" d={CURSOR} />
    </g>
  </svg>

  {#if showName}
    <span class="name" aria-hidden="true">harness <b>desktop</b></span>
  {/if}
</span>

<style>
  .logo {
    gap: var(--logo-gap, 0.5em);
  }

  svg {
    overflow: visible;
    flex-shrink: 0;
  }

  /* The family lockup: a muted, regular "harness" and the product name in bold. */
  .name {
    font-family: var(--font-ui);
    font-weight: 400;
    letter-spacing: -0.01em;
    line-height: 1;
    white-space: nowrap;
    color: var(--text-3);
  }

  .name b {
    font-weight: 650;
    color: var(--text);
  }

  .cursor.blink {
    animation: blink 1.05s steps(1, end) infinite;
  }

  @keyframes blink {
    0%,
    55% {
      opacity: 1;
    }
    56%,
    100% {
      opacity: 0;
    }
  }
</style>
