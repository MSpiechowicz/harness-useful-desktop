<script lang="ts">
  import { slide } from 'svelte/transition';

  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import Switch from '$lib/components/Switch.svelte';
  import { scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';
  import type { MemorySavePlace } from '$lib/ipc/bindings/MemorySavePlace';
  import { uid } from '$lib/utils/ids';

  import { REMOTE_SUBHEADING } from './remote-database';
  import type { SyncChoice } from './memory-sync';
  import SettingRow from './SettingRow.svelte';
  import UpdateNotice from './UpdateNotice.svelte';

  interface Props {
    /** What the installed Harness Memory can do. */
    remote: MemoryRemote;
    /** Looks at the installed Harness Memory again once the update ran. */
    onupdated: () => Promise<void>;
    /** A choice of the form changed. */
    onchange: (choice: SyncChoice, next: boolean) => void;
    /** The form's choices: the two copies the Sync button may make, and saving on this computer while the remote is unreachable. */
    copyToRemote: boolean;
    copyToLocal: boolean;
    offlineFallback: boolean;
    /** Where new notes and scripts go in "Remote + this computer", for agents and for what the user adds. */
    remoteLocalSaves: MemorySavePlace;
    /** The choice exists while notes or scripts are kept in "Remote + this computer". An older Harness Memory does not know it: it shows disabled with the update. */
    savePlaceShown: boolean;
    onsaveplace: (next: MemorySavePlace) => void;
  }

  let { remote, onupdated, onchange, copyToRemote, copyToLocal, offlineFallback, remoteLocalSaves, savePlaceShown, onsaveplace }: Props = $props();

  const heading = uid('memory-sync');

  const places = $derived<StripOption<MemorySavePlace>[]>([
    { value: 'remote', label: t('memory.editor.place.remote'), disabled: !remote.remoteLocalSavesSupported },
    { value: 'local', label: t('memory.editor.place.local'), disabled: !remote.remoteLocalSavesSupported },
  ]);

  /** The copies go between the remote database and this computer in a scope that reads both, and run on request: an older Harness Memory can do neither for the notes or for the scripts. */
  const copiesSupported = $derived((remote.teamAndLocalSupported && remote.notesSyncSupported) || (remote.scriptsSupported && remote.scriptsTeamAndLocalSupported && remote.scriptsSyncSupported));
</script>

<section aria-labelledby={heading}>
  <h3 id={heading} class={REMOTE_SUBHEADING}>{t('settings.memoryRemote.section.sync')}</h3>

  <div class="divide-y divide-line">
    <SettingRow
      title={t('settings.memoryRemote.copyToRemote.title')}
      description={copiesSupported ? t('settings.memoryRemote.copyToRemote.description') : t('settings.memoryRemote.copy.unsupported')}
      descriptions={[t('settings.memoryRemote.copyToRemote.description'), t('settings.memoryRemote.copy.unsupported')]}
    >
      {#snippet control()}
        <Switch label={t('settings.memoryRemote.copyToRemote.title')} disabled={!copiesSupported} bind:checked={() => copyToRemote, (next) => onchange('copyToRemote', next)} />
      {/snippet}
    </SettingRow>

    <SettingRow
      title={t('settings.memoryRemote.copyToLocal.title')}
      description={copiesSupported ? t('settings.memoryRemote.copyToLocal.description') : t('settings.memoryRemote.copy.unsupported')}
      descriptions={[t('settings.memoryRemote.copyToLocal.description'), t('settings.memoryRemote.copy.unsupported')]}
    >
      {#snippet control()}
        <Switch label={t('settings.memoryRemote.copyToLocal.title')} disabled={!copiesSupported} bind:checked={() => copyToLocal, (next) => onchange('copyToLocal', next)} />
      {/snippet}
    </SettingRow>

    <!-- Off, a note saved while the remote cannot be reached is not kept anywhere, so the description says it can be lost. -->
    <SettingRow title={t('settings.memoryRemote.offline.title')} description={t('settings.memoryRemote.offline.description')}>
      {#snippet control()}
        <Switch label={t('settings.memoryRemote.offline.title')} disabled={!remote.offlineFallbackSupported} bind:checked={() => offlineFallback, (next) => onchange('offlineFallback', next)} />
      {/snippet}
    </SettingRow>

    <!-- Only where a new entry may go to either side. It decides for agents and for what the user adds. -->
    {#if savePlaceShown}
      <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
        <SettingRow
          title={t('settings.memoryRemote.savePlace.title')}
          description={remote.remoteLocalSavesSupported ? t('settings.memoryRemote.savePlace.description') : t('settings.memoryRemote.savePlace.unsupported')}
          descriptions={[t('settings.memoryRemote.savePlace.description'), t('settings.memoryRemote.savePlace.unsupported')]}
        >
          {#snippet control()}
            <SegmentedControl label={t('settings.memoryRemote.savePlace.title')} options={places} bind:value={() => remoteLocalSaves, (next) => onsaveplace(next)} />
          {/snippet}
        </SettingRow>
      </div>
    {/if}
  </div>

  {#if !remote.offlineFallbackSupported}
    <UpdateNotice class="my-3" text={t('settings.memoryRemote.offline.unsupported')} onfinished={onupdated} />
  {:else if !copiesSupported}
    <UpdateNotice class="my-3" text={t('settings.memoryRemote.copy.unsupported')} onfinished={onupdated} />
  {:else if savePlaceShown && !remote.remoteLocalSavesSupported}
    <UpdateNotice class="my-3" text={t('settings.memoryRemote.savePlace.unsupported')} onfinished={onupdated} />
  {/if}
</section>
