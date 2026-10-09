<script lang="ts">
  import AccentGradient from '$lib/brand/AccentGradient.svelte';
  import { boxAttribute, CURSOR, GLYPH_BOX, H_BROKEN, ICON_BOX, LEG_PIVOT } from '$lib/brand/geometry';
  import LaunchMark from '$lib/brand/LaunchMark.svelte';
  import { motion } from '$lib/stores/motion.svelte';
  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  interface Props {
    /** Height of the mark in px; the name scales with it unless `nameSize` is given. */
    size?: number;
    /** Show the name next to the mark. */
    wordmark?: boolean;
    /** `lockup` mark + name, `mark` the glyph alone cropped flush, `icon` the glyph on a padded square (favicon shape). */
    variant?: 'lockup' | 'mark' | 'icon';
    /** `launch` plays the break-and-topple intro once; reduced motion and motion off show the finished mark. */
    animate?: 'launch' | null;
    /** Idle is completely still; working blinks the cursor. */
    state?: 'idle' | 'working';
    /** Font size of the name in px. */
    nameSize?: number;
    /** Called when the launch has finished (immediately when motion is reduced or off). */
    onfinish?: () => void;
    class?: string;
  }

  let {
    size = 24,
    wordmark = false,
    variant = wordmark ? 'lockup' : 'mark',
    animate = null,
    // Renamed locally: a binding called `state` would shadow the `$state` rune.
    state: activity = 'idle',
    nameSize = Math.round(size * 0.93 * 2) / 2,
    onfinish,
    class: className,
  }: Props = $props();

  const NAME = 'Harness Desktop';
  const gradient = uid('logo-gradient');
  const legGradient = uid('logo-leg-gradient');
  const blur = uid('logo-dust-blur');

  // The launch is skipped, never shortened: anything but full motion goes straight to the finished mark.
  const launching = $derived(animate === 'launch' && motion.level === 'full');
  const blinking = $derived(activity === 'working' && motion.level === 'full');
  const box = $derived(variant === 'icon' ? ICON_BOX : GLYPH_BOX);
  const width = $derived((size * box.width) / box.height);
  const showName = $derived(wordmark || variant === 'lockup');

  let finished = $state(false);
  const playing = $derived(launching && !finished);

  // A launch that cannot play (reduced motion, motion off) is complete at once.
  $effect(() => {
    if (animate === 'launch' && !launching) {
      onfinish?.();
    }
  });

  function finish(): void {
    finished = true;
    onfinish?.();
  }
</script>

<span
  class={cx('logo inline-flex items-center', className)}
  style:font-size="{nameSize}px"
  role="img"
  aria-label={NAME}
  data-state={activity}
  data-launching={playing ? '' : undefined}
>
  <svg {width} height={size} viewBox={boxAttribute(box)} aria-hidden="true">
    <defs>
      <AccentGradient id={gradient} />

      {#if playing}
        <AccentGradient id={legGradient} transform="rotate(90 {LEG_PIVOT.x} {LEG_PIVOT.y})" />
        <!--
          Dust: fractal noise tears the edges of each puff (displacement), punches wispy holes through it (alpha
          mask), and drifts while the dust spreads; a light blur softens what is left. No shape reads as a circle.
        -->
        <filter id={blur} filterUnits="userSpaceOnUse" x="10" y="26" width="160" height="104" color-interpolation-filters="sRGB">
          <feTurbulence type="fractalNoise" baseFrequency="0.07 0.11" numOctaves="3" seed="4" result="edges">
            <animate attributeName="baseFrequency" values="0.07 0.11;0.05 0.085" dur="3s" fill="freeze" />
          </feTurbulence>
          <feDisplacementMap in="SourceGraphic" in2="edges" scale="11" xChannelSelector="R" yChannelSelector="G" result="torn" />
          <feTurbulence type="fractalNoise" baseFrequency="0.1 0.15" numOctaves="2" seed="9" result="grain">
            <animate attributeName="baseFrequency" values="0.1 0.15;0.075 0.11" dur="3s" fill="freeze" />
          </feTurbulence>
          <feColorMatrix in="grain" type="matrix" values="0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  1.5 0 0 0 -0.12" result="wisps" />
          <feComposite in="torn" in2="wisps" operator="in" result="smoke" />
          <feGaussianBlur in="smoke" stdDeviation="1.8" />
        </filter>
      {/if}
    </defs>

    {#if playing}
      <LaunchMark {gradient} {legGradient} {blur} onfinish={finish} />
    {:else}
      <path d={H_BROKEN} fill="url(#{gradient})" />
      <path class={['cursor', blinking && 'blink']} data-part="cursor" d={CURSOR} fill="url(#{gradient})" />
    {/if}
  </svg>

  {#if showName}
    <span class={['name font-mono', playing && 'enter']} aria-hidden="true">
      <b>harness</b><span>desktop</span>
    </span>
  {/if}
</span>

<style>
  /* Sizes and weights of the name; the intro overrides them for its larger lockup. */
  .logo {
    gap: var(--logo-gap, 0.5em);
  }

  svg {
    overflow: visible;
    flex-shrink: 0;
  }

  .name {
    font-weight: var(--logo-name-weight, 650);
    letter-spacing: var(--logo-tracking, -0.02em);
    line-height: 1;
    white-space: nowrap;
    color: var(--text);
  }

  .name b {
    font-weight: inherit;
  }

  /*
   * Monospace gives every letter the same cell, so the thin regular "desktop" sits in more air than the bold
   * "harness" and the two words read as differently spaced. A real gap between them plus tighter tracking on
   * the light word gives both the same rhythm.
   */
  .name span {
    margin-left: 0.3em;
    letter-spacing: -0.06em;
    font-weight: 400;
    color: var(--text-3);
  }

  .name.enter {
    opacity: 0;
    animation: name-in 0.55s 2.13s cubic-bezier(0.2, 0.8, 0.2, 1) forwards;
  }

  .cursor.blink {
    animation: blink 1.05s steps(1, end) infinite;
  }

  @keyframes name-in {
    from {
      transform: translateX(-0.18em);
    }
    to {
      opacity: 1;
      transform: none;
    }
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
