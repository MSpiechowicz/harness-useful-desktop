<script lang="ts">
  import type { Snippet } from 'svelte';

  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
  import { cx } from '$lib/utils/cx';

  type Tone = 'neutral' | 'accent' | 'success' | 'warning' | 'danger' | 'info';

  interface Props {
    tone?: Tone;
    /** Harness badges take their color from the per-harness accent tokens and ignore `tone`. */
    harness?: HarnessId;
    dot?: boolean;
    class?: string;
    children: Snippet;
  }

  let { tone = 'neutral', harness, dot = false, class: className, children }: Props = $props();

  const TONES: Record<Tone, string> = {
    neutral: 'bg-surface-3 text-fg-2',
    accent: 'bg-accent-wash text-accent-text',
    success: 'bg-success-wash text-success',
    warning: 'bg-warning-wash text-warning',
    danger: 'bg-danger-wash text-danger',
    info: 'bg-info-wash text-info',
  };
  const HARNESSES: Record<HarnessId, string> = {
    omp: 'bg-omp-wash text-omp',
    pi: 'bg-pi-wash text-pi',
  };

  const colors = $derived(harness ? HARNESSES[harness] : TONES[tone]);
</script>

<span
  class={cx(
    'inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-xs leading-4 font-medium tracking-wide whitespace-nowrap',
    colors,
    className,
  )}
>
  {#if dot}
    <span class="size-1.5 rounded-full bg-current" aria-hidden="true"></span>
  {/if}
  {@render children()}
</span>
