<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MemoryNotesRouting } from '$lib/ipc/bindings/MemoryNotesRouting';
  import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';

  import KeptInRow from './KeptInRow.svelte';
  import { keptInHints, keptInOptions } from './kept-in';
  import UpdateNotice from './UpdateNotice.svelte';

  interface Props {
    /** What the installed Harness Memory can do, and which items it keeps. */
    remote: MemoryRemote;
    repositoryNotes: MemoryNotesRouting;
    globalNotes: MemoryNotesRouting;
    scripts: MemoryNotesRouting;
    /** Whether hooks and routines shared through the remote database are read here. Their own choice is shared or not, there is no remote-only place for them. */
    shareHooks: boolean;
    shareRoutines: boolean;
    onnotes: (scope: 'repositoryNotes' | 'globalNotes', routing: MemoryNotesRouting) => void;
    onscripts: (routing: MemoryNotesRouting) => void;
    onhooks: (shared: boolean) => void;
    onroutines: (shared: boolean) => void;
    /** Looks at the installed Harness Memory again once the update ran. */
    onupdated: () => Promise<void>;
  }

  let { remote, repositoryNotes, globalNotes, scripts, shareHooks, shareRoutines, onnotes, onscripts, onhooks, onroutines, onupdated }: Props = $props();

  /** An older Harness Memory has no third place: the option stays listed, greyed out, and the notice under the row says what to do. */
  const noteOptions = $derived(keptInOptions(remote.teamAndLocalSupported ? {} : { remoteLocal: null }));
  const noteHints = $derived(keptInHints('notes'));

  /** An older Harness Memory has no scripts to route at all, or none to read from both places. */
  const scriptOptions = $derived(keptInOptions(remote.scriptsSupported ? (remote.scriptsTeamAndLocalSupported ? {} : { remoteLocal: null }) : { remote: null, local: null, remoteLocal: null }));
  const scriptHints = $derived(keptInHints('scripts'));
  const mixMissing = $derived(remote.scriptsSupported && !remote.scriptsTeamAndLocalSupported);

  /** What other people share is read next to what is made here, never alone: Remote is shown, and says why it cannot be chosen. */
  const hookOptions = $derived(keptInOptions({ remote: t('settings.memoryRemote.keptIn.remoteOnlyDisabled.hooks') }));
  const hookHints = $derived(keptInHints('hooks'));
  const routineOptions = $derived(keptInOptions({ remote: t('settings.memoryRemote.keptIn.remoteOnlyDisabled.routines') }));
  const routineHints = $derived(keptInHints('routines'));
</script>

<KeptInRow
  title={t('settings.memoryRemote.repositoryNotes.title')}
  description={t('settings.memoryRemote.repositoryNotes.description')}
  options={noteOptions}
  hints={noteHints}
  value={repositoryNotes}
  onchange={(next) => onnotes('repositoryNotes', next)}
/>

<KeptInRow
  title={t('settings.memoryRemote.globalNotes.title')}
  description={t('settings.memoryRemote.globalNotes.description')}
  options={noteOptions}
  hints={noteHints}
  value={globalNotes}
  onchange={(next) => onnotes('globalNotes', next)}
/>

{#if !remote.teamAndLocalSupported}
  <UpdateNotice class="my-3" text={t('settings.memoryRemote.routing.unsupported')} onfinished={onupdated} />
{/if}

<KeptInRow
  title={t('settings.memoryRemote.scripts.title')}
  description={remote.scriptsSupported ? t('settings.memoryRemote.scripts.description') : t('settings.memoryRemote.scripts.unsupported')}
  descriptions={[t('settings.memoryRemote.scripts.description'), t('settings.memoryRemote.scripts.unsupported')]}
  options={scriptOptions}
  hints={scriptHints}
  value={scripts}
  onchange={onscripts}
/>

{#if mixMissing}
  <UpdateNotice class="my-3" text={t('settings.memoryRemote.scripts.mixUnsupported')} onfinished={onupdated} />
{/if}

<!-- Hooks and routines the remote database shares come from other people and run on this computer, so a row exists only for a server that keeps them. -->
{#if remote.hooksSupported}
  <KeptInRow
    title={t('settings.memoryRemote.hooks.title')}
    description={t('settings.memoryRemote.hooks.description')}
    options={hookOptions}
    hints={hookHints}
    value={shareHooks ? 'remoteLocal' : 'local'}
    onchange={(next) => onhooks(next === 'remoteLocal')}
  />
{/if}

{#if remote.routinesSupported}
  <KeptInRow
    title={t('settings.memoryRemote.routines.title')}
    description={t('settings.memoryRemote.routines.description')}
    options={routineOptions}
    hints={routineHints}
    value={shareRoutines ? 'remoteLocal' : 'local'}
    onchange={(next) => onroutines(next === 'remoteLocal')}
  />
{/if}
