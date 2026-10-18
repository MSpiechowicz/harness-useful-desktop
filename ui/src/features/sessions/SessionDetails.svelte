<script lang="ts">
  import SectionCard from '$lib/components/SectionCard.svelte';
  import StatGrid from '$lib/components/StatGrid.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
  import type { TranscriptStats } from '$lib/ipc/bindings/TranscriptStats';
  import { cx } from '$lib/utils/cx';
  import { formatBytes, formatCost, formatCount, formatDateTime } from '$lib/utils/format';

  import { formatDuration, formatTokens } from './session-format';
  import TouchedFiles from './TouchedFiles.svelte';

  interface Props {
    summary: SessionSummary;
    /** Totals of the session; `null` while they load or when the transcript has none. */
    stats: TranscriptStats | null;
    touched: string[];
    project: string;
    forks: number;
    /** Whether the harness has sessions that branch; without it the Forks row would only ever say 0. */
    showForks?: boolean;
  }

  let { summary, stats, touched, project, forks, showForks = true }: Props = $props();

  /** One colour per model in the split bar, in order of use. */
  const MODEL_COLORS = ['bg-accent', 'bg-success', 'bg-warning', 'bg-info', 'bg-danger'] as const;
  const NONE = '—';

  const models = $derived(stats?.models ?? []);
  const turns = $derived(models.reduce((sum, model) => sum + model.turns, 0));

  const overview = $derived([
    { label: t('sessions.details.messages'), value: stats ? formatCount(stats.messages) : NONE },
    { label: t('sessions.details.toolCalls'), value: stats ? formatCount(stats.toolCalls) : NONE },
    { label: t('sessions.details.context'), value: stats?.contextTokens != null ? formatTokens(stats.contextTokens) : NONE },
    { label: t('sessions.details.cost'), value: stats?.costUsd != null ? formatCost(stats.costUsd) : NONE },
  ]);

  const filesHeading = $derived(touched.length > 0 ? `${t('sessions.touched.label')} · ${formatCount(touched.length)}` : t('sessions.touched.label'));

  const started = $derived(stats?.startedAt != null ? formatDateTime(stats.startedAt) : NONE);
  const duration = $derived(stats?.startedAt != null && stats.endedAt != null ? formatDuration(stats.endedAt - stats.startedAt) : NONE);

  const session = $derived([
    { label: t('sessions.details.started'), value: started },
    { label: t('sessions.details.duration'), value: duration },
    { label: t('sessions.details.size'), value: formatBytes(summary.sizeBytes) },
    ...(showForks ? [{ label: t('sessions.details.forks'), value: formatCount(forks) }] : []),
  ]);
</script>

<!-- The same four sections in the same order whatever has loaded, in the body of the view's details panel. What is missing reads as a dash. -->
<SectionCard flat heading={t('sessions.details.overview')}>
  <StatGrid stats={overview} />
</SectionCard>

<SectionCard flat heading={t('sessions.details.models')}>
  {#if turns === 0}
    <p class="text-sm text-fg-3">{NONE}</p>
  {:else}
    <div class="mt-1.5 mb-2 flex h-1.5 overflow-hidden rounded-full bg-surface-3" aria-hidden="true">
      {#each models as model, index (model.model)}
        <span class={MODEL_COLORS[index % MODEL_COLORS.length]} style:width="{(model.turns / turns) * 100}%"></span>
      {/each}
    </div>
    <ul class="flex flex-col gap-1 text-xs text-fg-2">
      {#each models as model, index (model.model)}
        <li class="flex items-center gap-2">
          <span class={cx('size-2 shrink-0 rounded-full', MODEL_COLORS[index % MODEL_COLORS.length])} aria-hidden="true"></span>
          <span class="min-w-0 truncate" title={model.model}>{model.model}</span>
          <span class="tabular ml-auto shrink-0 text-fg-3">{tn('sessions.details.turns', model.turns)}</span>
        </li>
      {/each}
    </ul>
  {/if}
</SectionCard>

<SectionCard flat heading={t('sessions.details.session')}>
  <dl class="flex flex-col">
    {#each session as row (row.label)}
      <div class="flex justify-between gap-3 py-1 text-sm">
        <dt class="text-fg-2">{row.label}</dt>
        <dd class="tabular text-right text-fg">{row.value}</dd>
      </div>
    {/each}
  </dl>
</SectionCard>

<SectionCard flat heading={filesHeading}>
  <TouchedFiles {project} paths={touched} />
</SectionCard>
