<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';

  import FailureBox from './FailureBox.svelte';
  import { titleOfRow } from './notes-import';
  import { notesImport as flow } from './notes-import-state.svelte';
  import { spelledOut } from './script-format';

  const result = $derived(flow.result);
  const stats = $derived(
    result
      ? [
          { key: 'created', label: t('memory.import.result.created'), value: result.created },
          { key: 'updated', label: t('memory.import.result.updated'), value: result.updated },
          { key: 'replaced', label: t('memory.import.result.replaced'), value: result.replaced },
          { key: 'skipped', label: t('memory.import.result.skipped'), value: result.exists.length },
          { key: 'failed', label: t('memory.import.result.failed'), value: result.failed.length },
        ]
      : [],
  );

  /** The title of a row of the file, or its number when it has none. */
  function nameOf(index: number): string {
    const title = titleOfRow(flow.plan, index);

    return title === '' ? t('memory.import.row.number', { row: index + 1 }) : spelledOut(title);
  }

  function openNote(id: number): void {
    flow.open = false;
    viewState.noteId = id;
  }
</script>

{#if flow.stage === 'applying'}
  {@const progress = flow.progress}
  {@const total = progress?.total ?? 0}
  {@const done = progress?.done ?? 0}

  <div class="flex flex-col gap-3" role="status">
    <p class="tabular text-sm text-fg-2">{t('memory.import.progress', { done, total })}</p>

    <div role="progressbar" aria-label={t('memory.import.progress.label')} aria-valuemin={0} aria-valuemax={total} aria-valuenow={done} class="h-1.5 overflow-hidden rounded-full bg-surface-4">
      <div class="h-full rounded-full bg-accent transition-[width] duration-300 ease-linear" style:width="{total === 0 ? 0 : (done / total) * 100}%"></div>
    </div>
  </div>
{:else}
  <div class="flex flex-col gap-5">
    {#if flow.failure}
      <FailureBox message={flow.failure} />
    {/if}

    {#if result}
      <dl class="grid grid-cols-5 gap-3">
        {#each stats as stat (stat.key)}
          <div class="flex min-w-0 flex-col-reverse">
            <dt class="truncate text-xs text-fg-3">{stat.label}</dt>
            <dd class="tabular truncate text-lg font-semibold text-fg">{stat.value}</dd>
          </div>
        {/each}
      </dl>

      {#if result.exists.length > 0}
        <section class="flex flex-col gap-1" aria-label={t('memory.import.result.skipped.title')}>
          <h3 class="text-xs font-medium tracking-wider text-fg-3 uppercase">{t('memory.import.result.skipped.title')}</h3>
          <p class="text-sm text-fg-2">{t('memory.import.result.skipped.help')}</p>

          <ul class="mt-1 flex flex-col">
            {#each result.exists as entry (entry.index)}
              <li class="flex min-h-11 items-center justify-between gap-3 border-b border-line/60">
                <span class="min-w-0 text-sm text-fg [overflow-wrap:anywhere]">{nameOf(entry.index)}</span>

                {#if entry.id !== null}
                  {@const id = entry.id}
                  <Button variant="ghost" aria-label={t('memory.import.result.openNote', { title: nameOf(entry.index) })} onclick={() => openNote(id)}>{t('memory.import.result.open')}</Button>
                {/if}
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      {#if result.failed.length > 0}
        <section class="flex flex-col gap-1" aria-label={t('memory.import.result.failed.title')}>
          <h3 class="text-xs font-medium tracking-wider text-fg-3 uppercase">{t('memory.import.result.failed.title')}</h3>

          <ul class="mt-1 flex flex-col">
            {#each result.failed as entry (entry.index)}
              <li class="flex flex-col gap-0.5 border-b border-line/60 py-2.5">
                <span class="text-sm text-fg [overflow-wrap:anywhere]">{nameOf(entry.index)}</span>
                <span class="text-xs text-danger [overflow-wrap:anywhere]">{entry.message}</span>
              </li>
            {/each}
          </ul>
        </section>
      {/if}
    {/if}
  </div>
{/if}
