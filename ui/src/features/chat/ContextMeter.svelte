<script lang="ts">
  import Tooltip from '$lib/components/Tooltip.svelte';
  import type { ContextUsage } from '$lib/ipc/bindings/ContextUsage';
  import { cx } from '$lib/utils/cx';
  import { formatCount } from '$lib/utils/format';

  interface Props {
    usage: ContextUsage | null;
  }

  let { usage }: Props = $props();

  const percent = $derived(usage ? Math.min(100, Math.max(0, usage.percent)) : 0);
  const tone = $derived(percent >= 90 ? 'text-danger' : percent >= 75 ? 'text-warning' : 'text-accent');
  // A 16px ring: radius 6 with a 2px stroke.
  const CIRCUMFERENCE = 2 * Math.PI * 6;
  const detail = $derived(usage ? `${formatCount(usage.tokens)} of ${formatCount(usage.contextWindow)} tokens in context` : 'Context usage is not known yet');
</script>

<Tooltip text={detail} side="top">
  {#snippet children({ describedby })}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -- focusable so keyboard users can reach the tooltip -->
    <div
      role="meter"
      tabindex="0"
      aria-label="Context window used"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={usage ? Math.round(percent) : undefined}
      aria-valuetext={usage ? `${percent.toFixed(1)} percent` : 'unknown'}
      aria-describedby={describedby}
      class="inline-flex h-8 items-center gap-1.5 rounded-md px-1.5 text-xs text-fg-3"
    >
      <svg viewBox="0 0 16 16" class="size-4 -rotate-90" aria-hidden="true">
        <circle cx="8" cy="8" r="6" fill="none" stroke-width="2" class="stroke-surface-3" />
        <circle
          cx="8"
          cy="8"
          r="6"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-dasharray={CIRCUMFERENCE}
          stroke-dashoffset={CIRCUMFERENCE * (1 - percent / 100)}
          class={cx('transition-[stroke-dashoffset] duration-[var(--dur-slow)]', tone, percent === 0 && 'opacity-0')}
        />
      </svg>
      <span class="tabular">{usage ? `${percent.toFixed(percent < 10 ? 1 : 0)}%` : '—'}</span>
    </div>
  {/snippet}
</Tooltip>
