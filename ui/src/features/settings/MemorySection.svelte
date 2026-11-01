<script lang="ts">
  import { slide } from 'svelte/transition';

  import Switch from '$lib/components/Switch.svelte';
  import { scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';

  import ManualRemovalNote from './ManualRemovalNote.svelte';
  import MemoryGuardsSections from './MemoryGuardsSections.svelte';
  import MemorySearchRows from './MemorySearchRows.svelte';
  import { MEMORY_TEAM_DATABASE } from './memory-remote';
  import RemoteMemoryPanel from './RemoteMemoryPanel.svelte';
  import { readsRemote } from './routing-change';
  import SettingRow from './SettingRow.svelte';
  import SettingsSection from './SettingsSection.svelte';
  import TeamDatabaseRows from './TeamDatabaseRows.svelte';

  interface Props {
    id?: string;
  }

  let { id }: Props = $props();

  const remote = $derived(memoryRemote.value);
  /** Without Project memory no harness runs the server: remote memory keeps its saved settings but is greyed out. */
  const memoryOn = $derived(prefs.value?.memoryEnabled !== false);

  void memoryRemote.load();

  async function turnOff(): Promise<void> {
    if (!remote) {
      return;
    }

    await memoryRemote.save({ enabled: false, kind: remote.kind, url: null, token: null, repositoryNotes: remote.repositoryNotes, globalNotes: remote.globalNotes, scripts: remote.scripts, trustTeamScripts: readsRemote(remote.scripts) && remote.trustTeamScripts });

    // Without the remote database the notes and scripts are the local ones: the open one's id means something else.
    viewState.noteId = null;
    viewState.scriptId = null;
  }
</script>

<SettingsSection {id} title={t('settings.section.memory')}>
  <SettingRow title={t('settings.memory.title')} description={t('settings.memory.description')}>
    {#snippet control()}
      <Switch
        label={t('settings.memory.title')}
        disabled={prefs.memoryPending}
        bind:checked={() => prefs.value?.memoryEnabled ?? true, (memoryEnabled) => void prefs.setMemoryEnabled(memoryEnabled)}
      />
    {/snippet}
  </SettingRow>

  {#if prefs.memoryManualRemovals.length > 0}
    <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
      <ManualRemovalNote removals={prefs.memoryManualRemovals} ondismiss={() => prefs.dismissMemoryManualRemovals()} />
    </div>
  {/if}

  <MemorySearchRows />

  <TeamDatabaseRows copy={MEMORY_TEAM_DATABASE} saved={remote?.enabled ?? false} loaded={remote !== null} loadFailed={memoryRemote.loadFailed && remote === null} off={!memoryOn} {turnOff}>
    {#snippet panel()}
      {#if remote}<RemoteMemoryPanel {remote} />{/if}
    {/snippet}
  </TeamDatabaseRows>

  {#snippet after()}
    <MemoryGuardsSections />
  {/snippet}
</SettingsSection>
