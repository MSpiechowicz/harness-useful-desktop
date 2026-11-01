<script lang="ts">
  import { tick } from 'svelte';

  import Badge from '$lib/components/Badge.svelte';
  import { ICON } from '$lib/components/icon';
  import ProgressBar from '$lib/components/ProgressBar.svelte';
  import { rise } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeFileStatus } from '$lib/ipc/bindings/KnowledgeFileStatus';
  import { etaSeconds, fractionOf, type ImportRun } from '$lib/stores/knowledgeImport.svelte';
  import { cx } from '$lib/utils/cx';

  import { baseName, etaText, FILE_STATUSES, folderOf, STATUS_ICONS, STATUS_LABELS, STATUS_TONES } from './import-format';

  interface Props {
    run: ImportRun;
    running: boolean;
  }

  let { run, running }: Props = $props();

  let filter = $state<KnowledgeFileStatus | null>(null);
  let now = $state(Date.now());
  let list: HTMLUListElement | undefined = $state();

  // The clock of the ETA, while the import runs.
  $effect(() => {
    if (!running) {
      return;
    }

    const timer = setInterval(() => (now = Date.now()), 1000);

    return () => clearInterval(timer);
  });

  const counts = $derived(Object.fromEntries(FILE_STATUSES.map((status) => [status, run.rows.filter((row) => row.status === status).length])) as Record<KnowledgeFileStatus, number>);
  const shown = $derived(filter === null ? run.rows : run.rows.filter((row) => row.status === filter));
  const eta = $derived(running ? etaSeconds(run, now) : null);

  // A file that comes in while the list is not filtered is brought into view: the newest is the one at the bottom.
  $effect(() => {
    void run.rows.length;

    if (running && filter === null) {
      void tick().then(() => list?.scrollTo({ top: list.scrollHeight, behavior: 'smooth' }));
    }
  });

  // A result with no file left is not a filter to keep.
  $effect(() => {
    if (filter !== null && counts[filter] === 0) {
      filter = null;
    }
  });

  const CHIP = 'inline-flex h-7 items-center gap-1.5 rounded-full px-3 text-sm font-medium whitespace-nowrap transition-[background-color,color,transform] duration-[var(--dur-fast)] ease-[var(--ease-out-expo)] active:scale-95';
</script>

<div class="flex flex-col gap-4">
  <div class="flex flex-col gap-2">
    <ProgressBar label={t('knowledge.import.progress.label')} fraction={fractionOf(run)} />

    <div class="flex items-baseline justify-between gap-3 text-sm">
      <p class="tabular text-fg-2" role="status">{run.total === null ? t('knowledge.import.progress.reading') : t('knowledge.import.progress.files', { done: run.done, total: run.total })}</p>

      {#if eta !== null}
        <p class="tabular text-fg-3">{etaText(eta)}</p>
      {/if}
    </div>
  </div>

  {#if run.rows.length > 0}
    <div class="flex flex-wrap gap-2" role="group" aria-label={t('knowledge.import.chips.label')}>
      {#each FILE_STATUSES.filter((status) => counts[status] > 0) as status (status)}
        <button
          type="button"
          aria-pressed={filter === status}
          class={cx(CHIP, filter === status ? 'bg-accent-wash text-accent-text' : 'bg-surface-3 text-fg-2 hover:bg-hover hover:text-fg')}
          onclick={() => (filter = filter === status ? null : status)}
        >
          {t(STATUS_LABELS[status])}
          <span class="tabular text-fg-3">{counts[status]}</span>
        </button>
      {/each}
    </div>

    <ul bind:this={list} class="max-h-72 divide-y divide-line overflow-y-auto rounded-lg border border-line" aria-label={t('knowledge.import.rows.label')}>
      {#each shown as row, index (`${index}:${row.path}`)}
        {@const Icon = STATUS_ICONS[row.status]}
        <li in:rise={{ y: 6, duration: 220 }} class="flex min-h-12 items-center gap-3 px-3 py-2">
          <Icon size={ICON.sm} class={cx('shrink-0', row.status === 'failed' ? 'text-danger' : row.status === 'added' ? 'text-success' : 'text-fg-3')} aria-hidden="true" />

          <div class="flex min-w-0 flex-1 flex-col">
            <span class="truncate text-sm text-fg" title={row.path}>{baseName(row.path)}</span>
            <span class={cx('truncate text-xs', row.reason ? 'text-fg-2' : 'text-fg-3')}>{row.reason ?? folderOf(row.path)}</span>
          </div>

          {#if row.chunks !== null}<span class="tabular shrink-0 text-xs text-fg-3">{t('knowledge.import.chunks', { count: row.chunks })}</span>{/if}
          <Badge tone={STATUS_TONES[row.status]}>{t(STATUS_LABELS[row.status])}</Badge>
        </li>
      {:else}
        <li class="px-3 py-4 text-sm text-fg-3">{t('knowledge.import.rows.none')}</li>
      {/each}
    </ul>
  {/if}
</div>
