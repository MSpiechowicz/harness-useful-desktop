<script lang="ts">
  import { slide } from 'svelte/transition';

  import Switch from '$lib/components/Switch.svelte';
  import { scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { knowledgeSearch } from '$lib/stores/knowledgeEmbedder.svelte';
  import { knowledgeRemote } from '$lib/stores/knowledgeRemote.svelte';
  import { knowledgeStatus } from '$lib/stores/knowledgeStatus.svelte';
  import { KNOWLEDGE_STATUS_POLL_MS } from '$lib/stores/live';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { autoRefresh } from '$lib/utils/auto-refresh';

  import { KNOWLEDGE_EMBEDDER_COPY } from './embedder-status';
  import EmbedderRows from './EmbedderRows.svelte';
  import FamilyInstallRow from './FamilyInstallRow.svelte';
  import { KNOWLEDGE_TEAM_DATABASE } from './knowledge-remote';
  import KnowledgeRemotePanel from './KnowledgeRemotePanel.svelte';
  import ManualRemovalNote from './ManualRemovalNote.svelte';
  import SettingRow from './SettingRow.svelte';
  import SettingsSection from './SettingsSection.svelte';
  import TeamDatabaseRows from './TeamDatabaseRows.svelte';

  interface Props {
    id?: string;
  }

  let { id }: Props = $props();

  const remote = $derived(knowledgeRemote.value);
  /** Without Project knowledge no harness runs the server: the team database keeps its saved settings but is greyed out. */
  const knowledgeOn = $derived(prefs.value?.knowledgeEnabled !== false);
  const installable = $derived(knowledgeStatus.installable);

  void knowledgeRemote.load();

  // The server's state is read when the section opens and, while it is missing or old, again every few seconds: it may be getting installed elsewhere.
  $effect(() => {
    void knowledgeStatus.load();

    if (installable === null) {
      return undefined;
    }

    return autoRefresh(() => knowledgeStatus.load(), KNOWLEDGE_STATUS_POLL_MS);
  });

  // What the server supports (an embedder) is read again once an install made it current.
  async function installed(): Promise<void> {
    await Promise.all([knowledgeStatus.load(), knowledgeRemote.load()]);
  }

  async function turnOff(): Promise<void> {
    if (remote) {
      await knowledgeRemote.save({ enabled: false, kind: remote.kind, url: null, token: null });
    }
  }
</script>

<SettingsSection {id} title={t('knowledge.settings.section')}>
  <SettingRow title={t('knowledge.settings.title')} description={t('knowledge.settings.description')}>
    {#snippet control()}
      <Switch
        label={t('knowledge.settings.title')}
        disabled={prefs.knowledgePending}
        bind:checked={() => prefs.value?.knowledgeEnabled ?? true, (knowledgeEnabled) => void prefs.setKnowledgeEnabled(knowledgeEnabled)}
      />
    {/snippet}
  </SettingRow>

  {#if prefs.knowledgeManualRemovals.length > 0}
    <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
      <ManualRemovalNote removals={prefs.knowledgeManualRemovals} ondismiss={() => prefs.dismissKnowledgeManualRemovals()} />
    </div>
  {/if}

  {#if installable !== null}
    <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
      <FamilyInstallRow member="knowledge" update={installable === 'update'} onfinished={installed} />
    </div>
  {/if}

  <EmbedderRows search={knowledgeSearch} copy={KNOWLEDGE_EMBEDDER_COPY} />

  <TeamDatabaseRows copy={KNOWLEDGE_TEAM_DATABASE} saved={remote?.enabled ?? false} loaded={remote !== null} loadFailed={knowledgeRemote.loadFailed && remote === null} off={!knowledgeOn} {turnOff}>
    {#snippet panel()}
      {#if remote}<KnowledgeRemotePanel {remote} />{/if}
    {/snippet}
  </TeamDatabaseRows>
</SettingsSection>
