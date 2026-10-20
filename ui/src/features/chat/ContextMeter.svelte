<script lang="ts">
  import Tooltip from '$lib/components/Tooltip.svelte';
  import type { ContextUsage } from '$lib/ipc/bindings/ContextUsage';
  import { cx } from '$lib/utils/cx';
  import { formatCount, perLocale } from '$lib/utils/format';
  import { t } from '$lib/i18n/i18n.svelte';

  const ONE_DECIMAL = perLocale((locale) => new Intl.NumberFormat(locale, { minimumFractionDigits: 1, maximumFractionDigits: 1 }));
  const WHOLE = perLocale((locale) => new Intl.NumberFormat(locale, { maximumFractionDigits: 0 }));

  interface Props {
    usage: ContextUsage;
  }

  let { usage }: Props = $props();

  const percent = $derived(Math.min(100, Math.max(0, usage.percent)));
  const tone = $derived(percent >= 90 ? 'text-danger' : percent >= 75 ? 'text-warning' : 'text-accent');
  // A 16px ring: radius 6 with a 2px stroke.
  const CIRCUMFERENCE = 2 * Math.PI * 6;
  const detail = $derived(t('chat.context.detail', { used: formatCount(usage.tokens), total: formatCount(usage.contextWindow) }));
  const fraction = $derived(percent < 10 ? ONE_DECIMAL() : WHOLE());
</script>

<Tooltip text={detail} side="top">
  {#snippet children({ describedby })}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -- focusable so keyboard users can reach the tooltip -->
    <div
      role="meter"
      tabindex="0"
      aria-label={t('chat.context.label')}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(percent)}
      aria-valuetext={t('chat.context.valueText', { value: ONE_DECIMAL().format(percent) })}
      aria-describedby={describedby}
      class="inline-flex h-control items-center gap-1.5 rounded-md px-1.5 text-xs text-fg-3"
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
      <span class="tabular">{`${fraction.format(percent)}%`}</span>
    </div>
  {/snippet}
</Tooltip>
