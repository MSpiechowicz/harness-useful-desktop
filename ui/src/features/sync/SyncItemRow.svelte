<script lang="ts">
  import Checkbox from '$lib/components/Checkbox.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { SyncImportItem } from '$lib/ipc/bindings/SyncImportItem';

  import SyncCommand from './SyncCommand.svelte';
  import SyncNotices from './SyncNotices.svelte';
  import SyncRow from './SyncRow.svelte';
  import { commandLine, mcpEntryJson, statusChip, statusNote } from './sync-model';
  import type { ImportController } from './sync-import-state.svelte';

  interface Props {
    item: SyncImportItem;
    flow: ImportController;
    disabled: boolean;
  }

  let { item, flow, disabled }: Props = $props();

  const note = $derived(statusNote(item.status));
  const sub = $derived([item.version && item.status.type !== 'differentVersion' ? t('sync.versionShared', { version: item.version }) : null, note].filter((part) => part !== null).join(' · '));
  const unavailable = $derived(item.status.type === 'blocked' || item.status.type === 'invalid');
</script>

<SyncRow kind={item.kind} label={item.label} sub={sub || null} chip={statusChip(item.status)} dim={unavailable} reserve>
  {#snippet check()}
    <Checkbox
      label={t('sync.review.item', { name: item.label })}
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
      <SyncCommand title={t('sync.review.writes', { file: step.file })} text={mcpEntryJson(step.server)} />
    {/if}
  {/each}

  <SyncNotices runsProgram={item.runsProgram} envKeys={item.envKeys} headerKeys={item.headerKeys} notes={item.notes} />
</SyncRow>
