<script lang="ts">
  import { slide } from 'svelte/transition';

  import { scaled } from '$lib/design/motion';
  import Switch from '$lib/components/Switch.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MemoryCopyKind } from '$lib/ipc/bindings/MemoryCopyKind';
  import type { MemoryNotesRouting } from '$lib/ipc/bindings/MemoryNotesRouting';
  import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';
  import { MEMORY_REMOTE_SUPPORT_POLL_MS } from '$lib/stores/live';
  import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { autoRefresh } from '$lib/utils/auto-refresh';
  import { uid } from '$lib/utils/ids';

  import NotesCopyDialog from '../memory/NotesCopyDialog.svelte';

  import KeptInSection from './KeptInSection.svelte';
  import { MEMORY_REMOTE } from './memory-remote';
  import { REMOTE_SUBHEADING } from './remote-database';
  import { RemoteConnectionDraft } from './remote-connection.svelte';
  import RemoteDatabasePanel from './RemoteDatabasePanel.svelte';
  import RoutingChangeConfirm from './RoutingChangeConfirm.svelte';
  import { copyOffers, readsRemote, routesChanged, routingEffects, scopesMovedToTeam, type LocalHoldings, type Routes, type RoutingEffect } from './routing-change';
  import SettingRow from './SettingRow.svelte';
  import TrustRow from './TrustRow.svelte';
  import UpdateNotice from './UpdateNotice.svelte';

  interface Props {
    /** What the backend has saved; the form starts from it and keeps its own draft while it is open. */
    remote: MemoryRemote;
  }

  let { remote }: Props = $props();

  // The draft starts from what is saved; the secrets start empty and are never read back.
  const draft = new RemoteConnectionDraft(MEMORY_REMOTE, () => remote, (probe) => memoryRemote.test(probe), databaseChanged);

  // svelte-ignore state_referenced_locally
  let repositoryNotes = $state<MemoryNotesRouting>(remote.repositoryNotes);
  // svelte-ignore state_referenced_locally
  let globalNotes = $state<MemoryNotesRouting>(remote.globalNotes);
  // svelte-ignore state_referenced_locally
  let scripts = $state<MemoryNotesRouting>(remote.scripts);
  // svelte-ignore state_referenced_locally
  let trustTeamScripts = $state(remote.trustTeamScripts);
  // svelte-ignore state_referenced_locally
  let shareHooks = $state(remote.shareHooks);
  // svelte-ignore state_referenced_locally
  let trustTeamHooks = $state(remote.trustTeamHooks);
  // svelte-ignore state_referenced_locally
  let shareRoutines = $state(remote.shareRoutines);
  // svelte-ignore state_referenced_locally
  let trustTeamRoutines = $state(remote.trustTeamRoutines);
  /** A save that changes where notes are kept waits here for the user to read what it does. */
  let confirming = $state(false);
  let pendingEffects = $state.raw<RoutingEffect[]>([]);
  /** What a save that moved something to the remote database offers to copy from this computer, in the order of the dialogs. Each one leaves the queue when it is closed. */
  let copyQueue = $state.raw<MemoryCopyKind[]>([]);

  const base = uid('memory-remote');
  const headings = { keptIn: `${base}-kept-in`, trust: `${base}-trust` };

  /** Each trust question exists only while its item is kept in the team database. */
  const trustScripts = $derived(readsRemote(scripts));
  const trustHooks = $derived(remote.hooksSupported && shareHooks);
  const trustRoutines = $derived(remote.routinesSupported && shareRoutines);

  const routingChanged = $derived(
    repositoryNotes !== remote.repositoryNotes || globalNotes !== remote.globalNotes || scripts !== remote.scripts || trustTeamScripts !== remote.trustTeamScripts || shareHooks !== remote.shareHooks || trustTeamHooks !== remote.trustTeamHooks || shareRoutines !== remote.shareRoutines || trustTeamRoutines !== remote.trustTeamRoutines,
  );
  /** An older Harness Memory does not know Amazon DocumentDB: the choice stays listed but cannot be tested or saved. */
  const kindUnsupported = $derived(draft.kind === 'documentdb' && !remote.documentdbSupported);

  // While a warning about an old Harness Memory shows, it may be getting updated elsewhere: look again right away and then every few seconds. The form keeps its own draft, so only what the backend reports changes.
  const awaitingUpdate = $derived(kindUnsupported || !remote.teamAndLocalSupported || (remote.scriptsSupported && !remote.scriptsTeamAndLocalSupported));

  $effect(() => {
    if (!awaitingUpdate) {
      return undefined;
    }

    void memoryRemote.load();

    return autoRefresh(() => memoryRemote.load(), MEMORY_REMOTE_SUPPORT_POLL_MS);
  });

  /** Trusting every script, sharing hooks and trusting them are choices about one database: a new address or kind starts over, and the user turns them on again for the new one. */
  function databaseChanged(): void {
    trustTeamScripts = false;
    shareHooks = false;
    trustTeamHooks = false;
    shareRoutines = false;
    trustTeamRoutines = false;
  }

  /** Trust goes with sharing: hooks that are not shared have nothing to trust, and sharing again asks for the trust again. */
  function chooseShareHooks(next: boolean): void {
    shareHooks = next;
    trustTeamHooks = next && trustTeamHooks;
    draft.edited();
  }

  function chooseNotes(scope: 'repositoryNotes' | 'globalNotes', next: MemoryNotesRouting): void {
    if (scope === 'repositoryNotes') {
      repositoryNotes = next;
    } else {
      globalNotes = next;
    }

    draft.edited();
  }

  /** The same for routines. */
  function chooseShareRoutines(next: boolean): void {
    shareRoutines = next;
    trustTeamRoutines = next && trustTeamRoutines;
    draft.edited();
  }

  /** Where each scope is kept, as saved and as the form has it. */
  function savedRoutes(): Routes {
    return { repositoryNotes: remote.repositoryNotes, globalNotes: remote.globalNotes, scripts: remote.scripts };
  }

  function draftRoutes(): Routes {
    return { repositoryNotes, globalNotes, scripts };
  }

  /** What this computer holds that moving to the remote database would hide. */
  async function localHoldings(): Promise<LocalHoldings> {
    return { notes: await memoryRemote.localCounts(), scripts: await memoryRemote.scriptLocalCounts() };
  }

  /**
   * What saving changes in what the user sees, for the user to read first. A first connection has nothing saved to hide, and
   * what is saved on this computer is only counted when a scope moves.
   */
  async function effectsOfSave(): Promise<RoutingEffect[]> {
    if (!remote.enabled || !routesChanged(savedRoutes(), draftRoutes())) {
      return [];
    }

    return routingEffects(savedRoutes(), draftRoutes(), await localHoldings());
  }

  async function save(): Promise<void> {
    draft.busy = 'save';
    draft.status = null;

    let effects: RoutingEffect[];

    try {
      effects = await effectsOfSave();
    } catch (error) {
      draft.fail(error);
      draft.busy = null;

      return;
    }

    if (effects.length > 0) {
      draft.busy = null;
      pendingEffects = effects;
      confirming = true;

      return;
    }

    await commit();
  }

  /** Moving a scope to the remote database hides what is saved on this computer: when there is some, the user is offered to copy it, the notes first and then the scripts. */
  async function offerCopy(before: Routes, wasOn: boolean): Promise<void> {
    const moved = scopesMovedToTeam(before, wasOn, draftRoutes());

    copyQueue = moved.length > 0 ? copyOffers(moved, await localHoldings()) : [];
  }

  async function commit(): Promise<void> {
    draft.busy = 'save';
    draft.status = null;

    const before = savedRoutes();
    const wasOn = remote.enabled;

    try {
      await memoryRemote.save({
        enabled: true,
        kind: draft.kind,
        url: draft.typedUrl === '' ? null : draft.typedUrl,
        token: draft.tokenToSend(),
        repositoryNotes,
        globalNotes,
        scripts,
        // The switch only shows while the remote database's scripts are read, so a hidden one never stays on.
        trustTeamScripts: trustScripts && trustTeamScripts,
        // An older Harness Memory refuses a file with the hook keys, so they are not sent for it. The trust switch only shows while hooks are shared.
        ...(remote.hooksSupported ? { shareHooks, trustTeamHooks: shareHooks && trustTeamHooks } : {}),
        // The same for routines.
        ...(remote.routinesSupported ? { shareRoutines, trustTeamRoutines: shareRoutines && trustTeamRoutines } : {}),
      });

      draft.stored();

      // Ids shift when notes or scripts are kept elsewhere: the one that was open is not the same one any more.
      if (routesChanged(before, draftRoutes())) {
        viewState.noteId = null;
        viewState.scriptId = null;
      }
    } catch (error) {
      draft.fail(error);

      return;
    } finally {
      draft.busy = null;
    }

    // After the saved line shows and the spinner stops: counting what this computer holds waits for the database when it is down.
    try {
      await offerCopy(before, wasOn);
    } catch (error) {
      draft.fail(error);
    }
  }
</script>

<RemoteDatabasePanel {draft} blocked={kindUnsupported} otherChanges={routingChanged} onsave={() => void save()}>
  {#snippet notes(noteHint)}
    <!-- Amazon DocumentDB only answers from inside the cluster's network, so the VPN is the first thing to check when a test fails. -->
    {#if draft.kind === 'documentdb'}
      <div id={noteHint} transition:slide={{ duration: scaled(200) }} class="flex flex-col gap-1 overflow-hidden text-xs text-fg-3">
        <p>{t('settings.memoryRemote.documentdb.vpn')}</p>
        <p>{t('settings.memoryRemote.documentdb.help')}</p>
      </div>
    {/if}
    {#if kindUnsupported}
      <UpdateNotice class="mt-2.5" text={t('memory.unavailable.outdated.title')} onfinished={() => memoryRemote.load()} />
    {/if}
  {/snippet}

  {#snippet sections()}
    <section aria-labelledby={headings.keptIn}>
      <h3 id={headings.keptIn} class={REMOTE_SUBHEADING}>{t('settings.memoryRemote.section.keptIn')}</h3>
      <div class="divide-y divide-line">
        <KeptInSection
          {remote}
          {repositoryNotes}
          {globalNotes}
          {scripts}
          {shareHooks}
          {shareRoutines}
          onnotes={chooseNotes}
          onscripts={(next) => ((scripts = next), draft.edited())}
          onhooks={chooseShareHooks}
          onroutines={chooseShareRoutines}
          onupdated={() => memoryRemote.load()}
        />
      </div>
    </section>

    <!-- Only what is kept in the team database comes from other people, so the question only exists while it is. -->
    {#if trustScripts || trustHooks || trustRoutines}
      <section transition:slide={{ duration: scaled(200) }} aria-labelledby={headings.trust} class="overflow-hidden">
        <h3 id={headings.trust} class={REMOTE_SUBHEADING}>{t('settings.memoryRemote.section.trust')}</h3>
        <div class="divide-y divide-line">
          {#if trustScripts}
            <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
              <SettingRow
                title={t('settings.memoryRemote.trustTeam.title')}
                description={remote.trustTeamScriptsSupported ? t('settings.memoryRemote.trustTeam.description') : t('settings.memoryRemote.trustTeam.unsupported')}
                descriptions={[t('settings.memoryRemote.trustTeam.description'), t('settings.memoryRemote.trustTeam.unsupported')]}
              >
                {#snippet control()}
                  <Switch
                    label={t('settings.memoryRemote.trustTeam.title')}
                    disabled={!remote.trustTeamScriptsSupported}
                    bind:checked={() => trustTeamScripts, (next) => ((trustTeamScripts = next), draft.edited())}
                  />
                {/snippet}
              </SettingRow>
            </div>
          {/if}

          {#if trustHooks}
            <TrustRow title={t('settings.memoryRemote.trustHooks.title')} description={t('settings.memoryRemote.trustHooks.description')} checked={trustTeamHooks} onchange={(next) => ((trustTeamHooks = next), draft.edited())} />
          {/if}

          {#if trustRoutines}
            <TrustRow title={t('settings.memoryRemote.trustRoutines.title')} description={t('settings.memoryRemote.trustRoutines.description')} checked={trustTeamRoutines} onchange={(next) => ((trustTeamRoutines = next), draft.edited())} />
          {/if}
        </div>
      </section>
    {/if}
  {/snippet}
</RemoteDatabasePanel>

<RoutingChangeConfirm bind:open={confirming} effects={pendingEffects} onconfirm={() => void commit()} />

<!-- The dialogs wait in a queue: the notes are copied first, then the scripts. -->
<NotesCopyDialog kind="notes" open={copyQueue[0] === 'notes'} onclose={() => (copyQueue = copyQueue.slice(1))} />
<NotesCopyDialog kind="scripts" open={copyQueue[0] === 'scripts'} onclose={() => (copyQueue = copyQueue.slice(1))} />
