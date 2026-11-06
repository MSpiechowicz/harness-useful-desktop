<script lang="ts">
  import Folder from '@lucide/svelte/icons/folder';
  import Pause from '@lucide/svelte/icons/pause';
  import Plus from '@lucide/svelte/icons/plus';

  import Button from '$lib/components/Button.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import { ICON } from '$lib/components/icon';
  import Switch from '$lib/components/Switch.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { GuardKind } from '$lib/stores/memory-chat';
  import { memoryChat } from '$lib/stores/memoryChat.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { cx } from '$lib/utils/cx';

  import { ASK_PREF, folderName, GUARD_TEXT, pickRepositoryFolder } from './memory-guards';
  import SettingRow from './SettingRow.svelte';
  import SettingsSection from './SettingsSection.svelte';

  interface Props {
    kind: GuardKind;
  }

  let { kind }: Props = $props();

  let picking = $state(false);

  /** Turning saving on for every project waits for the user's yes: it reaches projects they have not opened yet. */
  let confirmingEvery = $state(false);

  const text = $derived(GUARD_TEXT[kind]);
  const guard = $derived(memoryChat.guards[kind]);
  const everyDescription = $derived(t(guard.everyProject ? text.everyOn : text.every));
  const askPref = $derived(ASK_PREF[kind]);

  /** Off is immediate. On asks first, and the switch stays off until the user said yes. */
  function setEvery(on: boolean): void {
    if (on) {
      confirmingEvery = true;

      return;
    }

    void memoryChat.setEveryProject(kind, false);
  }

  async function add(): Promise<void> {
    picking = true;

    try {
      const folder = await pickRepositoryFolder();

      if (folder !== null) {
        await memoryChat.addRepo(kind, folder);
      }
    } finally {
      picking = false;
    }
  }
</script>

<SettingsSection title={t(text.title)}>
  {#snippet above()}
    {#if guard.paused}
      <div class="flex items-center gap-3 rounded-md bg-warning-wash py-1.5 pr-1.5 pl-3.5 text-sm text-warning" role="status">
        <Pause size={14} aria-hidden="true" class="shrink-0" />
        {t(text.paused)}
        <Button variant="warning" class="ml-auto brightness-125" onclick={() => void memoryChat.setGlobalPause(kind, false)}>{t('memory.guards.resume')}</Button>
      </div>
    {/if}
  {/snippet}

  <SettingRow title={t('memory.guards.settings.pause')} description={t(text.pause)}>
    {#snippet control()}
      <Switch label={`${t(text.title)}: ${t('memory.guards.settings.pause')}`} bind:checked={() => guard.paused, (paused) => void memoryChat.setGlobalPause(kind, paused)} />
    {/snippet}
  </SettingRow>

  <SettingRow title={t('memory.guards.ask')} description={t(text.ask)}>
    {#snippet control()}
      <Switch label={`${t(text.title)}: ${t('memory.guards.ask')}`} bind:checked={() => prefs.value?.[askPref] ?? false, (on) => void prefs.update({ [askPref]: on })} />
    {/snippet}
  </SettingRow>

  <SettingRow title={t('memory.guards.settings.every')} description={everyDescription} descriptions={[t(text.every), t(text.everyOn)]}>
    {#snippet control()}
      <Switch
        label={`${t(text.title)}: ${t('memory.guards.settings.every')}`}
        bind:checked={() => guard.everyProject, setEvery}
      />
    {/snippet}
  </SettingRow>

  <div>
    <SettingRow title={t(text.repos)} description={t(text.reposHelp)}>
      {#snippet control()}
        <Button loading={picking} onclick={() => void add()}>
          {#snippet leading()}<Plus size={14} aria-hidden="true" />{/snippet}
          {t('memory.guards.settings.repos.add')}
        </Button>
      {/snippet}
    </SettingRow>

    <ul class="chat-radius -mt-1 mb-4 divide-y divide-line overflow-hidden border border-line" aria-label={t(text.repos)}>
      {#each guard.onRepos as repo (repo)}
        <li class="flex min-h-12 items-center gap-3 py-1.5 pr-1.5 pl-3.5 text-sm">
          <Folder size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />
          <div class="min-w-0 flex-1">
            <div class="flex min-w-0 items-baseline gap-3">
              <span class={cx('shrink-0 font-medium', guard.deadRepos.includes(repo) ? 'text-fg-3' : 'text-fg')}>{folderName(repo)}</span>
              <span class="min-w-0 flex-1 truncate font-mono text-xs text-fg-3" title={repo}>{repo}</span>
            </div>
            {#if guard.changedRepos.includes(repo)}
              <p class="mt-0.5 text-xs text-warning">{t('memory.guards.settings.repos.changed')}</p>
            {:else if guard.deadRepos.includes(repo)}
              <p class="mt-0.5 text-xs text-warning">{t(guard.everyProject ? 'memory.guards.settings.repos.deadEvery' : 'memory.guards.settings.repos.dead')}</p>
            {/if}
          </div>
          {#if guard.changedRepos.includes(repo)}
            <Button variant="ghost" aria-label={t('memory.guards.settings.repos.turnOnAgainAria', { name: folderName(repo) })} onclick={() => void memoryChat.addRepo(kind, repo)}>{t('memory.guards.settings.repos.turnOnAgain')}</Button>
          {/if}
          <Button variant="ghost" aria-label={t('memory.guards.settings.repos.removeAria', { name: folderName(repo) })} onclick={() => void memoryChat.setRepoOn(kind, repo, false)}>{t('memory.guards.settings.repos.remove')}</Button>
        </li>
      {:else}
        <li class="flex items-center py-1.5 pr-1.5 pl-3.5 text-sm text-fg-3"><span class="flex h-control items-center">{t(text.none)}</span></li>
      {/each}
    </ul>
  </div>
</SettingsSection>

<ConfirmDialog
  bind:open={confirmingEvery}
  tone="primary"
  title={t(text.everyTitle)}
  description={t(text.everyBody)}
  confirmLabel={t('memory.guards.settings.every.confirm')}
  cancelLabel={t('memory.guards.settings.repos.cancel')}
  onconfirm={() => memoryChat.setEveryProject(kind, true)}
/>
