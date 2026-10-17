<script lang="ts">
  import { motion } from '$lib/stores/motion.svelte';
  import { cx } from '$lib/utils/cx';

  type Tone = 'success' | 'warning' | 'danger' | 'neutral' | 'accent';

  interface Props {
    tone: Tone;
    /**
     * A live indicator: a copy of the dot grows out of it and fades, the same ping the Harness dashboard uses. The
     * dot itself never moves. Off unless motion is full.
     */
    live?: boolean;
    /**
     * The dot stands before a word. The middle of a line box lies a little above the middle of lowercase letters, which
     * is what the eye measures, so the dot is set 1px lower to sit on it. Left off for a dot with no text beside it.
     */
    beforeText?: boolean;
    class?: string;
  }

  let { tone, live = false, beforeText = false, class: className }: Props = $props();

  const TONES: Record<Tone, string> = {
    success: 'bg-success',
    warning: 'bg-warning',
    danger: 'bg-danger',
    neutral: 'bg-fg-3',
    accent: 'bg-accent',
  };
</script>

<span class={cx('relative inline-flex size-[8px] shrink-0', beforeText && 'top-px', className)} aria-hidden="true">
  {#if live && motion.level === 'full'}
    <span class={cx('absolute inline-flex size-full animate-ping rounded-full opacity-60', TONES[tone])}></span>
  {/if}
  <span class={cx('relative inline-flex size-full rounded-full', TONES[tone])}></span>
</span>
