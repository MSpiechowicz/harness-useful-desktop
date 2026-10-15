<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
  import { uid } from '$lib/utils/ids';

  import SyncCommand from './SyncCommand.svelte';
  import SyncGroupHead from './SyncGroupHead.svelte';
  import SyncRow from './SyncRow.svelte';
  import { commandLine } from './sync-model';
  import type { ImportController, RunRow } from './sync-import-state.svelte';

  interface Props {
    flow: ImportController;
  }

  let { flow }: Props = $props();

  const base = uid('sync-result');

  /** The rows of each harness, harnesses in the order the run reached them. */
  const groups = $derived.by(() => {
    const byHarness = new Map<HarnessId, RunRow[]>();

    for (const row of flow.runRows) {
      byHarness.set(row.harness, [...(byHarness.get(row.harness) ?? []), row]);
    }

    return [...byHarness];
  });
</script>

<div class="flex flex-col gap-6">
  {#each groups as [harness, rows] (harness)}
    <section aria-labelledby="{base}-{harness}">
      <SyncGroupHead id="{base}-{harness}" {harness} />

      <ul>
        {#each rows as row (row.id)}
          <SyncRow kind={row.kind} label={row.label} sub={row.note} chip={row.chip}>
            {#if row.summary && row.chip.tone === 'danger'}
              <p class="text-sm text-fg-2 [overflow-wrap:anywhere]">{row.summary}</p>
            {/if}

            {#if row.argv.length > 0 && row.chip.tone === 'danger'}
              <SyncCommand title={t('sync.result.command')} text={commandLine(row.argv)} />
            {/if}
          </SyncRow>
        {/each}
      </ul>
    </section>
  {/each}
</div>
