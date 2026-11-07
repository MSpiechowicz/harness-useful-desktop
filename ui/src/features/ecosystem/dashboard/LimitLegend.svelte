<script lang="ts">
  import type { LimitMode } from '$lib/dashboard/limit-dial';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  interface Props {
    /** The provider's color, solid and pale. */
    color: string;
    mode: LimitMode;
    /** The battery always draws what is left, so its solid cells are what is left at the reset, not the use so far. */
    battery: boolean;
    class?: string;
  }

  let { color, mode, battery, class: className }: Props = $props();

  const now = $derived(battery ? t('ecosystem.limits.legend.chargeReset') : t(mode === 'used' ? 'ecosystem.limits.legend.usedNow' : 'ecosystem.limits.legend.leftNow'));
  const ahead = $derived(t(mode === 'used' ? 'ecosystem.limits.legend.pacedUsed' : 'ecosystem.limits.legend.pacedLeft'));
</script>

<!-- The key to the charts, in the header line beside the plan rather than under the charts, where it read as part of them. -->
<span class={cx('flex shrink-0 items-center gap-3 text-2xs text-fg-3', className)}>
  <span class="inline-flex items-center gap-1.5">
    <i class="inline-block h-[var(--limit-swatch-h)] w-[var(--limit-swatch-w)] rounded-[var(--limit-block-radius)]" style:background={color}></i>{now}
  </span>
  <span class="inline-flex items-center gap-1.5">
    <i class="inline-block h-[var(--limit-swatch-h)] w-[var(--limit-swatch-w)] rounded-[var(--limit-block-radius)]" style:background={color} style:opacity="0.35"></i>{ahead}
  </span>
</span>
