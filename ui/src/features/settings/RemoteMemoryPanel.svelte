<script lang="ts">
  import { slide } from 'svelte/transition';

  import { scaled } from '$lib/design/motion';
  import Switch from '$lib/components/Switch.svelte';
  import Button from '$lib/components/Button.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MemoryCopyKind } from '$lib/ipc/bindings/MemoryCopyKind';
  import type { MemoryNotesRouting } from '$lib/ipc/bindings/MemoryNotesRouting';
  import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';
  import type { MemoryRemoteKind } from '$lib/ipc/bindings/MemoryRemoteKind';
  import { IpcFailure } from '$lib/ipc/transport';
  import { MEMORY_REMOTE_SUPPORT_POLL_MS } from '$lib/stores/live';
  import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { autoRefresh } from '$lib/utils/auto-refresh';
  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  import NotesCopyDialog from '../memory/NotesCopyDialog.svelte';

  import DatabasePicker from './DatabasePicker.svelte';
  import KeptInSection from './KeptInSection.svelte';
  import { savedUrlFits, takesToken, URL_PLACEHOLDERS, URL_PROBLEMS, urlMatchesKind } from './memory-remote';
  import RoutingChangeConfirm from './RoutingChangeConfirm.svelte';
  import { copyOffers, readsRemote, routesChanged, routingEffects, scopesMovedToTeam, type LocalHoldings, type Routes, type RoutingEffect } from './routing-change';
  import SecretField from './SecretField.svelte';
  import SettingRow from './SettingRow.svelte';
  import TrustRow from './TrustRow.svelte';
  import UpdateNotice from './UpdateNotice.svelte';

  interface Props {
    /** What the backend has saved; the form starts from it and keeps its own draft while it is open. */
    remote: MemoryRemote;
  }

  let { remote }: Props = $props();

  // The draft starts from what is saved; the secrets start empty and are never read back.
  // svelte-ignore state_referenced_locally
  let kind = $state<MemoryRemoteKind>(remote.kind ?? 'postgres');
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
  let url = $state('');
  let token = $state('');
  let touched = $state(false);
  let busy = $state<'test' | 'save' | null>(null);
  let status = $state<{ ok: boolean; text: string } | null>(null);
  /** A save that changes where notes are kept waits here for the user to read what it does. */
  let confirming = $state(false);
  let pendingEffects = $state.raw<RoutingEffect[]>([]);
  /** What a save that moved something to the remote database offers to copy from this computer, in the order of the dialogs. Each one leaves the queue when it is closed. */
  let copyQueue = $state.raw<MemoryCopyKind[]>([]);

  /** The sub-headings of the panel look like the section headings of Settings, one step smaller in the card. */
  const HEADING = 'pt-4 text-sm font-medium tracking-wider text-fg-3 uppercase';

  const base = uid('memory-remote');
  const headings = { connection: `${base}-connection`, keptIn: `${base}-kept-in`, trust: `${base}-trust` };
  const urlHint = `${base}-url`;
  const noteHint = `${base}-note`;

  /** Each trust question exists only while its item is kept in the team database. */
  const trustScripts = $derived(readsRemote(scripts));
  const trustHooks = $derived(remote.hooksSupported && shareHooks);
  const trustRoutines = $derived(remote.routinesSupported && shareRoutines);

  const typedUrl = $derived(url.trim());
  const typedToken = $derived(token.trim());
  const sameKind = $derived(remote.kind === kind);
  const urlInvalid = $derived(typedUrl !== '' && !urlMatchesKind(kind, typedUrl));
  /** A typed URL has to fit the database; without one, the saved URL stands in if it is for this database. */
  const savedUrlStands = $derived(savedUrlFits(kind, remote.urlRedacted));
  const urlUsable = $derived(typedUrl !== '' ? !urlInvalid : savedUrlStands);
  const changed = $derived(
    !remote.enabled || !sameKind || typedUrl !== '' || typedToken !== '' || repositoryNotes !== remote.repositoryNotes || globalNotes !== remote.globalNotes || scripts !== remote.scripts || trustTeamScripts !== remote.trustTeamScripts || shareHooks !== remote.shareHooks || trustTeamHooks !== remote.trustTeamHooks || shareRoutines !== remote.shareRoutines || trustTeamRoutines !== remote.trustTeamRoutines,
  );
  /** An older Harness Memory does not know Amazon DocumentDB: the choice stays listed but cannot be tested or saved. */
  const kindUnsupported = $derived(kind === 'documentdb' && !remote.documentdbSupported);

  // While a warning about an old Harness Memory shows, it may be getting updated elsewhere: look again right away and then every few seconds. The form keeps its own draft, so only what the backend reports changes.
  const awaitingUpdate = $derived(kindUnsupported || !remote.teamAndLocalSupported || (remote.scriptsSupported && !remote.scriptsTeamAndLocalSupported));

  $effect(() => {
    if (!awaitingUpdate) {
      return undefined;
    }

    void memoryRemote.load();

    return autoRefresh(() => memoryRemote.load(), MEMORY_REMOTE_SUPPORT_POLL_MS);
  });
  const canSave = $derived(busy === null && urlUsable && changed && !kindUnsupported);
  const canTest = $derived(busy === null && urlUsable && !kindUnsupported);
  const showSavedUrl = $derived(savedUrlStands);

  function edited(): void {
    status = null;
  }

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
    edited();
  }

  function chooseNotes(scope: 'repositoryNotes' | 'globalNotes', next: MemoryNotesRouting): void {
    if (scope === 'repositoryNotes') {
      repositoryNotes = next;
    } else {
      globalNotes = next;
    }

    edited();
  }

  /** The same for routines. */
  function chooseShareRoutines(next: boolean): void {
    shareRoutines = next;
    trustTeamRoutines = next && trustTeamRoutines;
    edited();
  }

  function typeUrl(next: string): void {
    if (next !== url) {
      databaseChanged();
    }

    url = next;
  }

  function chooseKind(next: MemoryRemoteKind): void {
    if (next === kind) {
      return;
    }

    databaseChanged();
    kind = next;
    url = '';
    token = '';
    touched = false;
    edited();
  }

  /** The token to send: null keeps the saved one, an empty string clears it (when the database no longer takes one). */
  function tokenToSend(): string | null {
    if (!takesToken(kind)) {
      return remote.hasToken && remote.kind !== null && takesToken(remote.kind) ? '' : null;
    }

    return typedToken === '' ? null : typedToken;
  }

  function failure(error: unknown): string {
    return IpcFailure.from(error).message;
  }

  async function test(): Promise<void> {
    busy = 'test';
    status = null;

    try {
      const result = await memoryRemote.test({ kind, url: typedUrl === '' ? null : typedUrl, token: takesToken(kind) && typedToken !== '' ? typedToken : null });

      status = { ok: result.ok, text: result.message };
    } catch (error) {
      status = { ok: false, text: failure(error) };
    } finally {
      busy = null;
    }
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
    busy = 'save';
    status = null;

    const effects = await effectsOfSave();

    if (effects.length > 0) {
      busy = null;
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
    busy = 'save';
    status = null;

    const before = savedRoutes();
    const wasOn = remote.enabled;

    try {
      await memoryRemote.save({
        enabled: true,
        kind,
        url: typedUrl === '' ? null : typedUrl,
        token: tokenToSend(),
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

      // The secrets leave the form as soon as they are stored.
      url = '';
      token = '';
      touched = false;
      status = { ok: true, text: t('settings.memoryRemote.saved') };

      // Ids shift when notes or scripts are kept elsewhere: the one that was open is not the same one any more.
      if (routesChanged(before, draftRoutes())) {
        viewState.noteId = null;
        viewState.scriptId = null;
      }

      await offerCopy(before, wasOn);
    } catch (error) {
      status = { ok: false, text: failure(error) };
    } finally {
      busy = null;
    }
  }
</script>

<div class="divide-y divide-line">
  <section aria-labelledby={headings.connection}>
    <h3 id={headings.connection} class={HEADING}>{t('settings.memoryRemote.section.connection')}</h3>
    <div class="divide-y divide-line">
        <SettingRow
          title={t('settings.memoryRemote.database.title')}
          description={t('settings.memoryRemote.database.description')}
          points={[t('settings.memoryRemote.restart'), t('settings.memoryRemote.stored', { path: remote.configPath })]}
        >
          {#snippet control()}
            <DatabasePicker value={kind} onchange={chooseKind} />
          {/snippet}
        </SettingRow>

        <div class="flex flex-col gap-4 py-4">
          <div class="flex flex-col gap-1.5">
            <p class="text-base font-medium text-fg">{t('settings.memoryRemote.url.label')}</p>
            <SecretField
              bind:value={() => url, typeUrl}
              label={t('settings.memoryRemote.url.label')}
              placeholder={showSavedUrl ? t('settings.memoryRemote.url.keep') : t(URL_PLACEHOLDERS[kind])}
              showLabel={t('settings.memoryRemote.url.show')}
              hideLabel={t('settings.memoryRemote.url.hide')}
              invalid={urlInvalid && touched}
              describedby={`${urlHint} ${noteHint}`}
              onblur={() => (touched ||= typedUrl !== '')}
            />
            {#if urlInvalid && touched}
              <p id={urlHint} class="text-xs text-danger" role="alert">{t(URL_PROBLEMS[kind])}</p>
            {:else if showSavedUrl}
              <p id={urlHint} class="text-xs text-fg-3">{t('settings.memoryRemote.url.saved', { url: remote.urlRedacted ?? '' })}</p>
            {/if}
            <!-- Amazon DocumentDB only answers from inside the cluster's network, so the VPN is the first thing to check when a test fails. -->
            {#if kind === 'documentdb'}
              <div id={noteHint} transition:slide={{ duration: scaled(200) }} class="flex flex-col gap-1 overflow-hidden text-xs text-fg-3">
                <p>{t('settings.memoryRemote.documentdb.vpn')}</p>
                <p>{t('settings.memoryRemote.documentdb.help')}</p>
              </div>
            {/if}
            {#if kindUnsupported}
              <UpdateNotice class="mt-2.5" text={t('memory.unavailable.outdated.title')} onfinished={() => memoryRemote.load()} />
            {/if}
          </div>

          {#if takesToken(kind)}
            <div class="flex flex-col gap-1.5">
              <p class="text-base font-medium text-fg">{t('settings.memoryRemote.token.label')}</p>
              <SecretField
                bind:value={token}
                label={t('settings.memoryRemote.token.label')}
                placeholder={sameKind && remote.hasToken ? t('settings.memoryRemote.token.keep') : t('settings.memoryRemote.token.placeholder')}
                showLabel={t('settings.memoryRemote.token.show')}
                hideLabel={t('settings.memoryRemote.token.hide')}
              />
            </div>
          {/if}
        </div>
    </div>
  </section>

  <section aria-labelledby={headings.keptIn}>
    <h3 id={headings.keptIn} class={HEADING}>{t('settings.memoryRemote.section.keptIn')}</h3>
    <div class="divide-y divide-line">
      <KeptInSection
        {remote}
        {repositoryNotes}
        {globalNotes}
        {scripts}
        {shareHooks}
        {shareRoutines}
        onnotes={chooseNotes}
        onscripts={(next) => ((scripts = next), edited())}
        onhooks={chooseShareHooks}
        onroutines={chooseShareRoutines}
        onupdated={() => memoryRemote.load()}
      />
    </div>
  </section>

  <!-- Only what is kept in the team database comes from other people, so the question only exists while it is. -->
  {#if trustScripts || trustHooks || trustRoutines}
    <section transition:slide={{ duration: scaled(200) }} aria-labelledby={headings.trust} class="overflow-hidden">
      <h3 id={headings.trust} class={HEADING}>{t('settings.memoryRemote.section.trust')}</h3>
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
                  bind:checked={() => trustTeamScripts, (next) => ((trustTeamScripts = next), edited())}
                />
              {/snippet}
            </SettingRow>
          </div>
        {/if}

        {#if trustHooks}
          <TrustRow title={t('settings.memoryRemote.trustHooks.title')} description={t('settings.memoryRemote.trustHooks.description')} checked={trustTeamHooks} onchange={(next) => ((trustTeamHooks = next), edited())} />
        {/if}

        {#if trustRoutines}
          <TrustRow title={t('settings.memoryRemote.trustRoutines.title')} description={t('settings.memoryRemote.trustRoutines.description')} checked={trustTeamRoutines} onchange={(next) => ((trustTeamRoutines = next), edited())} />
        {/if}
      </div>
    </section>
  {/if}

  <div class="flex flex-col gap-3 py-4">
    <div class="flex flex-wrap items-center gap-2">
      <Button loading={busy === 'test'} disabled={!canTest} onclick={() => void test()}>{t('settings.memoryRemote.test')}</Button>
      <Button variant="primary" loading={busy === 'save'} disabled={!canSave} onclick={() => void save()}>{t('settings.memoryRemote.save')}</Button>
    </div>

    <div role="status">
      {#if status}
        <p class={cx('text-sm', status.ok ? 'text-success' : 'text-danger')}>{status.text}</p>
      {/if}
    </div>

    {#if remote.problem}
      <p class="text-sm text-danger" role="alert">{t('settings.memoryRemote.problem', { problem: remote.problem })}</p>
    {/if}
  </div>
</div>

<RoutingChangeConfirm bind:open={confirming} effects={pendingEffects} onconfirm={() => void commit()} />

<!-- The dialogs wait in a queue: the notes are copied first, then the scripts. -->
<NotesCopyDialog kind="notes" open={copyQueue[0] === 'notes'} onclose={() => (copyQueue = copyQueue.slice(1))} />
<NotesCopyDialog kind="scripts" open={copyQueue[0] === 'scripts'} onclose={() => (copyQueue = copyQueue.slice(1))} />
