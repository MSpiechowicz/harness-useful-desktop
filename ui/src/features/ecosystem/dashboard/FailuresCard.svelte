<script lang="ts">
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import { formatShare } from '$lib/dashboard/format';
  import type { Failures } from '$lib/dashboard/models';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { dashboard, type PanelRange } from '$lib/stores/dashboard.svelte';
  import { formatCount } from '$lib/utils/format';

  import { causeColor } from '../charts/error-colors';
  import type { ShareSegment } from '../charts/share';
  import Waffle from '../charts/Waffle.svelte';
  import { causeShares, causeWords as words } from './failures';
  import { phraseOf } from './ranges';
  import CardEmpty from './CardEmpty.svelte';
  import SlotCard from './SlotCard.svelte';

  interface Props {
    range: PanelRange;
    index?: number;
  }

  let { range, index = 0 }: Props = $props();

  const phrase = $derived(phraseOf(dashboard.failures.data?.range, range));

  function segments(failures: Failures): ShareSegment[] {
    return causeShares(failures.causes).map((part) => {
      const label = part.folded ? t('ecosystem.failures.rest') : words(part.key);
      const folded = part.folded ? part.causes.map(words).join(', ') : undefined;
      const failures = tn('ecosystem.count.failure', part.count);
      const share = formatShare(part.share);
      const named = folded ? t('ecosystem.failures.otherNamed', { causes: folded }) : label;

      return {
        key: part.key,
        label,
        value: part.count,
        valueLabel: formatCount(part.count),
        shareLabel: formatShare(part.share),
        note: folded,
        description:
          part.tools.length > 0
            ? t('ecosystem.failures.descriptionTools', { label: named, failures, share, tools: part.tools.join(', ') })
            : t('ecosystem.failures.description', { label: named, failures, share }),
        color: causeColor(part.folded ? null : part.key),
      };
    });
  }
</script>

<SlotCard title={t('ecosystem.failures.title')} subtitle={t('ecosystem.failures.subtitle', { phrase })} slot={dashboard.failures} onretry={() => void dashboard.openPanel(true)} {index} class="@container">
  {#snippet children(failures)}
    {#if failures.failed === 0}
      <CardEmpty icon={CircleCheck} tone="success" title={t('ecosystem.failures.empty')} />
    {:else}
      <dl class="tabular flex flex-wrap gap-x-10 gap-y-3 text-sm">
        <div class="flex flex-col"><dt class="text-fg-3">{t('ecosystem.failures.failed')}</dt><dd class="text-xl font-semibold text-fg">{failures.failed} <span class="text-sm font-normal text-fg-3">{t('ecosystem.failures.ofCalls', { total: formatCount(failures.calls) })}</span></dd></div>
        <div class="flex flex-col"><dt class="text-fg-3">{t('ecosystem.failures.errorRate')}</dt><dd class="text-xl font-semibold text-fg">{formatShare(failures.errorRate)}</dd></div>
        <div class="flex flex-col"><dt class="text-fg-3">{t('ecosystem.failures.declined')}</dt><dd class="text-xl font-semibold text-fg">{failures.declined}</dd></div>
      </dl>

      <div class="mt-2">
        <h4 class="mb-3 text-sm font-medium text-fg-3">{t('ecosystem.failures.causes')}</h4>
        <Waffle segments={segments(failures)} label={t('ecosystem.failures.chart')} />
      </div>
    {/if}
  {/snippet}
</SlotCard>
