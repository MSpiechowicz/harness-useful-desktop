<script lang="ts">
  import { leftTone } from '$lib/dashboard/limits';
  import { LIMIT_COLORS, limitFill } from '$lib/dashboard/palette';
  import { t } from '$lib/i18n/i18n.svelte';
  import { motion } from '$lib/stores/motion.svelte';
  import { theme } from '$lib/stores/theme.svelte';
  import { cx } from '$lib/utils/cx';
  import { growOnMount } from './grow.svelte';

  interface Props {
    /** The full sentence a screen reader hears: "Claude Max 5-hour limit: 67% left, on pace to last until the reset". */
    label: string;
    /** What is left, 0..1. */
    left: number;
    /** The share of the window's time that is left, 0..1; null draws no tick. */
    timeLeft?: number | null;
    /** The provider whose color the bar wears while plenty is left. */
    provider: string;
  }

  let { label, left, timeLeft = null, provider }: Props = $props();

  const mode = $derived(theme.resolved);
  const tone = $derived(leftTone(left));
  const warnInk = $derived(LIMIT_COLORS.warnInk[mode]);

  /** Less left than time left means the limit runs out before the window does. */
  const short = $derived(timeLeft !== null && left < timeLeft);
  const growth = growOnMount();
</script>

<div
  role="meter"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={Math.round(left * 100)}
  aria-valuetext={t('ecosystem.limits.left', { percent: Math.round(left * 100) })}
  class="relative"
>
  <div class="relative h-2 overflow-hidden rounded-full" style:background={LIMIT_COLORS.track[mode]} aria-hidden="true">
    <span
      class="absolute inset-y-0 left-0 origin-left rounded-full transition-[width,transform] duration-[var(--dur-enter)] ease-[var(--ease-out-expo)]"
      style:width={left > 0 ? `max(${left * 100}%, 4px)` : '0'}
      style:background={limitFill(tone, provider, mode)}
      style:transform="scaleX({growth.grown ? 1 : 0})"
    ></span>

    {#if timeLeft !== null && short}
      <span
        class="absolute inset-y-0"
        style:background="repeating-linear-gradient(-45deg, color-mix(in srgb, {warnInk} 45%, transparent) 0 2px, transparent 2px 5px)"
        style:left="{left * 100}%"
        style:width="{(timeLeft - left) * 100}%"
      ></span>
    {/if}
  </div>

  {#if timeLeft !== null}
    <span class="absolute top-1/2 h-0 w-0" style:left="{timeLeft * 100}%" style:color={short ? warnInk : LIMIT_COLORS.ink[mode]} aria-hidden="true">
      <span class={cx('tick absolute h-3.5 w-[3px] -translate-x-1/2 -translate-y-1/2 rounded-full bg-current', motion.level === 'full' && 'tick-ripple')}></span>
    </span>
  {/if}
</div>

<style>
  .tick {
    box-shadow:
      0 0 0 1.5px var(--surface-1),
      0 0 6px 1px color-mix(in srgb, currentColor 45%, transparent);
  }

  /* A ring that swells off the tick now and then; only transform and opacity move. */
  .tick-ripple::after {
    content: '';
    position: absolute;
    inset: -2px;
    border: 1px solid currentColor;
    border-radius: 9999px;
    opacity: 0;
    pointer-events: none;
    animation: tick-ripple 3s cubic-bezier(0.22, 1, 0.36, 1) infinite;
  }

  @keyframes tick-ripple {
    0% {
      transform: scale(1);
      opacity: 0.6;
    }

    75%,
    100% {
      transform: scale(2.4, 1.3);
      opacity: 0;
    }
  }
</style>
