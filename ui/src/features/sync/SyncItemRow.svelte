<script lang="ts">
  import Checkbox from '$lib/components/Checkbox.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
  import type { SyncImportItem } from '$lib/ipc/bindings/SyncImportItem';

  import SyncCommand from './SyncCommand.svelte';
  import SyncNotices from './SyncNotices.svelte';
  import SyncRow from './SyncRow.svelte';
  import { commandLine, itemLabel, mcpEntryJson, showsVersion, statusChip, statusNote } from './sync-model';
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
</script>

<SyncRow kind={item.kind} {label} sub={sub || null} chip={statusChip(item.status)} dim={unavailable} reserve>
  {#snippet check()}
    <Checkbox
      label={t('sync.review.item', { name: label })}
      checked={flow.isSelected(item.id)}
      disabled={disabled || flow.locked(item)}
      onchange={(on) => flow.toggle(item.id, on)}
    />
  {/snippet}

  {#if item.detail && item.steps.length === 0}
    <code class="block text-xs leading-snug text-fg-3 [overflow-wrap:anywhere]">{item.detail}</code>
  {/if}

  {#each item.steps as step, index (index)}
    {#if step.type === 'cli'}
      <SyncCommand title={t('sync.review.runs')} text={commandLine(step.command)} />
    {:else}
      <SyncCommand title={t('sync.review.writes', { file: step.file })} text={mcpEntryJson(step.server, harness)} />
    {/if}
  {/each}

  <SyncNotices runsProgram={item.runsProgram} envKeys={item.envKeys} headerKeys={item.headerKeys} notes={item.notes} />
</SyncRow>
