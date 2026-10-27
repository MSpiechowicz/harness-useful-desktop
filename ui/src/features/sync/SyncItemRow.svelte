<script lang="ts">
  import Checkbox from '$lib/components/Checkbox.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
  import type { SyncImportItem } from '$lib/ipc/bindings/SyncImportItem';
  import { harness as harnesses } from '$lib/stores/harness.svelte';

  import SyncCommand from './SyncCommand.svelte';
  import SyncNotices from './SyncNotices.svelte';
  import SyncRow from './SyncRow.svelte';
  import { cliAddCommand, commandLine, itemLabel, mcpEntryJson, showsVersion, statusChip, statusNote } from './sync-model';
  import type { ImportController } from './sync-import-state.svelte';

  interface Props {
    item: SyncImportItem;
    /** The harness the item belongs to, which decides how its name and its MCP entry are written. */
    harness: HarnessId;
    flow: ImportController;
    disabled: boolean;
  }

  let { item, harness, flow, disabled }: Props = $props();

  const label = $derived(itemLabel(harness, item));
  const note = $derived(statusNote(item.status, item.notes));
  const sub = $derived([showsVersion(harness, item) && item.status.type !== 'differentVersion' ? t('sync.versionShared', { version: item.version }) : null, note].filter((part) => part !== null).join(' · '));
  const unavailable = $derived(item.status.type === 'blocked' || item.status.type === 'invalid');
  const replaceRow = $derived(item.status.type === 'differentDefinition');
  // A replace that keeps the entry's saved values has no full entry to write: when the address or program changes, the row writes
  // the whole entry like a new server and is noted like one. A CLI row does not say which of the two it is, so it makes no promise.
  const keepsValues = $derived(replaceRow && item.steps.some((step) => step.type === 'mcpReplace'));
  const viaCli = $derived.by(() => {
    const command = cliAddCommand(item);

    return command ? { command, harness: harnesses.get(harness)?.info.name ?? harness } : null;
  });
</script>

<SyncRow kind={item.kind} {label} sub={sub || null} detail={item.steps.length === 0 ? item.detail : null} chip={statusChip(item.status)} dim={unavailable} reserve>
  {#snippet check()}
    <Checkbox
      label={t(replaceRow ? 'sync.review.replaceItem' : 'sync.review.item', { name: label })}
      checked={flow.isSelected(item.id)}
      disabled={disabled || flow.locked(item)}
      onchange={(on) => flow.toggle(item.id, on)}
    />
  {/snippet}

  {#each item.steps as step, index (index)}
    {#if step.type === 'cli'}
      <SyncCommand title={t('sync.review.runs')} text={commandLine(step.command)} />
    {:else if step.type === 'mcpReplace'}
      <SyncCommand title={t('sync.review.replaces', { file: step.file })} text={mcpEntryJson(step.server, harness, harnesses.openCodeGeneration, true)} />
    {:else}
      <SyncCommand title={t('sync.review.writes', { file: step.file })} text={mcpEntryJson(step.server, harness, harnesses.openCodeGeneration)} />
    {/if}
  {/each}

  <SyncNotices runsProgram={item.runsProgram} envKeys={item.envKeys} headerKeys={item.headerKeys} notes={item.notes} {viaCli} replacing={keepsValues} />
</SyncRow>
