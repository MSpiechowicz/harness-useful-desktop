<script lang="ts">
  import { onMount } from 'svelte';

  import { motion } from '$lib/stores/motion.svelte';
  import { cx } from '$lib/utils/cx';

  import { meterTone, TONE_FILLS, TONE_WORDS } from './colors';

  interface Props {
    /** What is measured: "Claude 5h window". */
    label: string;
    /** 0..100, or null when there is no reading. */
    percent: number | null;
    /** The text the value reads as, e.g. "42%". */
    valueText: string;
    /** Where a projection ends (0..100); drawn as a hatched stretch after the fill. */
    projectedPercent?: number | null;
  }

  let { label, percent, valueText, projectedPercent = null }: Props = $props();

  const clamped = $derived(percent === null ? 0 : Math.min(100, Math.max(0, percent)));
  const tone = $derived(meterTone(clamped));
  const word = $derived(percent === null ? null : TONE_WORDS[tone]);

  let grown = $state(motion.level !== 'full');

  onMount(() => {
    if (!grown) {
      const frame = requestAnimationFrame(() => (grown = true));

      return () => cancelAnimationFrame(frame);
    }
  });
</script>

<div
  role="meter"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={percent === null ? undefined : Math.round(clamped)}
  aria-valuetext={percent === null ? 'no reading' : `${valueText}${word ? `, ${word.toLowerCase()}` : ''}`}
  class="flex items-center gap-3"
>
  <span class="relative h-2 min-w-0 flex-1 overflow-hidden rounded-full bg-surface-3" aria-hidden="true">
    {#if projectedPercent !== null && projectedPercent > clamped}
      <span
        class="absolute inset-y-0 left-0 rounded-full bg-[repeating-linear-gradient(135deg,var(--text-3)_0_2px,transparent_2px_5px)] opacity-60"
        style:width="{Math.min(100, projectedPercent)}%"
      ></span>
    {/if}
    <span
      class={cx('absolute inset-y-0 left-0 origin-left rounded-full transition-[width,transform] duration-[var(--dur-enter)] ease-[var(--ease-out-expo)]', TONE_FILLS[tone])}
      style:width="{clamped}%"
      style:transform="scaleX({grown ? 1 : 0})"
    ></span>
  </span>

  <span class="tabular w-12 shrink-0 text-right text-sm text-fg">{percent === null ? '—' : valueText}</span>

  {#if word}
    <span class={cx('shrink-0 text-xs font-medium', tone === 'danger' ? 'text-danger' : 'text-warning')}>{word}</span>
  {/if}
</div>
