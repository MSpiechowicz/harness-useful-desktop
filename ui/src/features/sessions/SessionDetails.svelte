<script lang="ts">
  import Card from '$lib/components/Card.svelte';
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
    class?: string;
  }

  let { summary, stats, touched, project, forks, class: className }: Props = $props();

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

  const started = $derived(stats?.startedAt != null ? formatDateTime(stats.startedAt) : NONE);
  const duration = $derived(stats?.startedAt != null && stats.endedAt != null ? formatDuration(stats.endedAt - stats.startedAt) : NONE);

  const session = $derived([
    { label: t('sessions.details.started'), value: started },
    { label: t('sessions.details.duration'), value: duration },
    { label: t('sessions.details.size'), value: formatBytes(summary.sizeBytes) },
    { label: t('sessions.details.forks'), value: formatCount(forks) },
  ]);
</script>

<!-- The same four cards in the same order whatever has loaded; what is missing reads as a dash. -->
<aside aria-label={t('sessions.details.label')} class={cx('flex flex-col gap-3 overflow-y-auto p-4', className)}>
  <Card as="section" padded={false} class="p-3.5">
    {@render title(t('sessions.details.overview'))}
    <dl class="grid grid-cols-2 gap-x-3 gap-y-3">
      {#each overview as stat (stat.label)}
        <div class="flex flex-col-reverse">
          <dt class="text-xs text-fg-3">{stat.label}</dt>
          <dd class="tabular text-lg font-semibold text-fg">{stat.value}</dd>
        </div>
      {/each}
    </dl>
  </Card>

  <Card as="section" padded={false} class="p-3.5">
    {@render title(t('sessions.details.models'))}
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
  </Card>

  <Card as="section" padded={false} class="p-3.5">
    {@render title(t('sessions.details.session'))}
    <dl class="flex flex-col">
      {#each session as row (row.label)}
        <div class="flex justify-between gap-3 py-1 text-sm">
          <dt class="text-fg-2">{row.label}</dt>
          <dd class="tabular text-right text-fg">{row.value}</dd>
        </div>
      {/each}
    </dl>
  </Card>

  <Card as="section" padded={false} class="p-3.5">
    {@render title(t('sessions.touched.label'), touched.length > 0 ? formatCount(touched.length) : null)}
    <TouchedFiles {project} paths={touched} />
  </Card>
</aside>

{#snippet title(text: string, count: string | null = null)}
  <h3 class="mb-2.5 flex items-center gap-1.5 text-xs font-medium tracking-wider text-fg-3 uppercase">
    {text}
    {#if count}<span class="tabular tracking-normal">· {count}</span>{/if}
  </h3>
{/snippet}
