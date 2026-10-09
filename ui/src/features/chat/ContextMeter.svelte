<script lang="ts">
  import Tooltip from '$lib/components/Tooltip.svelte';
  import type { ContextUsage } from '$lib/ipc/bindings/ContextUsage';
  import { cx } from '$lib/utils/cx';

  interface Props {
    usage: ContextUsage | null;
  }

  let { usage }: Props = $props();

  const percent = $derived(usage ? Math.min(100, Math.max(0, usage.percent)) : 0);
  const tone = $derived(percent >= 90 ? 'bg-danger' : percent >= 75 ? 'bg-warning' : 'bg-accent');
  const detail = $derived(usage ? `${usage.tokens.toLocaleString()} of ${usage.contextWindow.toLocaleString()} tokens in context` : 'Context usage is not known yet');
</script>

<Tooltip text={detail} side="bottom">
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
      class="inline-flex h-8 items-center gap-2 rounded-md px-2 text-xs text-fg-3"
    >
      <span class="h-1.5 w-14 overflow-hidden rounded-full bg-surface-3" aria-hidden="true">
        <span class={cx('block h-full rounded-full transition-[width] duration-[var(--dur-slow)]', tone)} style:width="{percent}%"></span>
      </span>
      <span class="tabular w-10">{usage ? `${percent.toFixed(percent < 10 ? 1 : 0)}%` : '—'}</span>
    </div>
  {/snippet}
</Tooltip>
