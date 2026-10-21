<script lang="ts">
  import { slide } from 'svelte/transition';

  import Button from '$lib/components/Button.svelte';
  import Switch from '$lib/components/Switch.svelte';
  import { cx } from '$lib/utils/cx';
  import { scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { IpcFailure } from '$lib/ipc/transport';
  import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';

  import ManualRemovalNote from './ManualRemovalNote.svelte';
  import MemoryGuardsSections from './MemoryGuardsSections.svelte';
  import RemoteMemoryPanel from './RemoteMemoryPanel.svelte';
  import { readsRemote } from './routing-change';
  import SettingRow from './SettingRow.svelte';
  import SettingsSection from './SettingsSection.svelte';

  interface Props {
    id?: string;
  }

  let { id }: Props = $props();

  const remote = $derived(memoryRemote.value);
  /** Without Project memory no harness runs the server: remote memory keeps its saved settings but is greyed out. */
  const memoryOn = $derived(prefs.value?.memoryEnabled !== false);
  const saved = $derived(remote?.enabled ?? false);

  /** The switch position: it can be on before anything is saved, while the connection is still being entered. */
  let wanted = $state(false);
  let confirmingOff = $state(false);
  let turningOff = $state(false);
  let offFailure = $state<string | null>(null);

  void memoryRemote.load();

  // Follow the backend: what is saved decides the switch whenever it changes.
  $effect(() => {
    wanted = saved;
    confirmingOff = false;
  });

  function toggle(next: boolean): void {
    offFailure = null;

    if (next) {
      confirmingOff = false;
      wanted = true;

      return;
    }

    // Switching off something that is saved changes every teammate's memory, so it asks first.
    if (saved) {
      confirmingOff = !confirmingOff;

      return;
    }

    wanted = false;
  }

  async function turnOff(): Promise<void> {
    if (!remote) {
      return;
    }

    turningOff = true;
    offFailure = null;

    try {
      await memoryRemote.save({ enabled: false, kind: remote.kind, url: null, token: null, repositoryNotes: remote.repositoryNotes, globalNotes: remote.globalNotes, scripts: remote.scripts, trustTeamScripts: readsRemote(remote.scripts) && remote.trustTeamScripts });

      // Without the remote database the notes and scripts are the local ones: the open one's id means something else.
      viewState.noteId = null;
      viewState.scriptId = null;
    } catch (error) {
      offFailure = IpcFailure.from(error).message;
    } finally {
      turningOff = false;
    }
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

  <SettingRow title={t('settings.memoryRemote.title')} description={t('settings.memoryRemote.description')}>
    {#snippet control()}
      <Switch label={t('settings.memoryRemote.title')} disabled={remote === null || turningOff || !memoryOn} bind:checked={() => wanted, toggle} />
    {/snippet}
  </SettingRow>

  {#if memoryRemote.loadFailed && remote === null}
    <p class="py-4 text-sm text-danger" role="alert">{t('settings.memoryRemote.loadFailed')}</p>
  {/if}

  {#if confirmingOff}
    <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
      <div class="flex items-center justify-between gap-6 py-4" role="alertdialog" aria-label={t('settings.memoryRemote.off.title')}>
        <div class="flex min-w-0 flex-col gap-0.5">
          <p class="text-base font-medium text-fg">{t('settings.memoryRemote.off.title')}</p>
          <p class="text-sm text-fg-2">{t('settings.memoryRemote.off.body')}</p>
          {#if offFailure}<p class="text-sm text-danger" role="alert">{offFailure}</p>{/if}
        </div>
        <div class="flex shrink-0 items-center gap-2">
          <Button variant="ghost" disabled={turningOff} onclick={() => (confirmingOff = false)}>{t('settings.memoryRemote.off.cancel')}</Button>
          <Button variant="dangerQuiet" loading={turningOff} onclick={() => void turnOff()}>{t('settings.memoryRemote.off.confirm')}</Button>
        </div>
      </div>
    </div>
  {/if}

  {#if wanted && remote}
    <!-- With Project memory off the saved connection stays as it is, only greyed out, so turning it back on just works. -->
    <div transition:slide={{ duration: scaled(200) }} class={cx('overflow-hidden transition-opacity duration-[var(--dur-fast)]', !memoryOn && 'opacity-50')} inert={!memoryOn}>
      <RemoteMemoryPanel {remote} />
    </div>
  {/if}

  {#snippet after()}
    <MemoryGuardsSections />
  {/snippet}
</SettingsSection>
