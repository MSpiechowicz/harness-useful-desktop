<script lang="ts">
  import { slide } from 'svelte/transition';

  import { scaled } from '$lib/design/motion';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import Switch from '$lib/components/Switch.svelte';
  import type { StripOption } from '$lib/components/strip';
  import Button from '$lib/components/Button.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MemoryNotesTarget } from '$lib/ipc/bindings/MemoryNotesTarget';
  import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';
  import type { MemoryRemoteKind } from '$lib/ipc/bindings/MemoryRemoteKind';
  import { IpcFailure } from '$lib/ipc/transport';
  import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  import DatabasePicker from './DatabasePicker.svelte';
  import { takesToken, URL_PLACEHOLDERS, URL_PROBLEMS, urlMatchesKind } from './memory-remote';
  import SecretField from './SecretField.svelte';
  import SettingRow from './SettingRow.svelte';

  interface Props {
    /** What the backend has saved; the form starts from it and keeps its own draft while it is open. */
    remote: MemoryRemote;
  }

  let { remote }: Props = $props();

  // The draft starts from what is saved; the secrets start empty and are never read back.
  // svelte-ignore state_referenced_locally
  let kind = $state<MemoryRemoteKind>(remote.kind ?? 'postgres');
  // svelte-ignore state_referenced_locally
  let repositoryNotes = $state<MemoryNotesTarget>(remote.repositoryNotes);
  // svelte-ignore state_referenced_locally
  let globalNotes = $state<MemoryNotesTarget>(remote.globalNotes);
  // svelte-ignore state_referenced_locally
  let scripts = $state<MemoryNotesTarget>(remote.scripts);
  // svelte-ignore state_referenced_locally
  let trustTeamScripts = $state(remote.trustTeamScripts);
  let url = $state('');
  let token = $state('');
  let touched = $state(false);
  let busy = $state<'test' | 'save' | null>(null);
  let status = $state<{ ok: boolean; text: string } | null>(null);

  const base = uid('memory-remote');
  const urlHint = `${base}-url`;

  const TARGETS: StripOption<MemoryNotesTarget>[] = [
    {
      value: 'remote',
      get label() {
        return t('settings.memoryRemote.target.remote');
      },
    },
    {
      value: 'local',
      get label() {
        return t('settings.memoryRemote.target.local');
      },
    },
  ];

  /** An older Harness Memory has no scripts to route: the row stays, greyed out, and says what to do. */
  const SCRIPT_TARGETS = $derived(TARGETS.map((option) => ({ ...option, disabled: !remote.scriptsSupported })));

  const typedUrl = $derived(url.trim());
  const typedToken = $derived(token.trim());
  const sameKind = $derived(remote.kind === kind);
  const urlInvalid = $derived(typedUrl !== '' && !urlMatchesKind(kind, typedUrl));
  /** A typed URL has to fit the database; without one, the saved URL stands in if it is for this database. */
  const urlUsable = $derived(typedUrl !== '' ? !urlInvalid : sameKind && remote.urlRedacted !== null);
  const changed = $derived(
    !remote.enabled || !sameKind || typedUrl !== '' || typedToken !== '' || repositoryNotes !== remote.repositoryNotes || globalNotes !== remote.globalNotes || scripts !== remote.scripts || trustTeamScripts !== remote.trustTeamScripts,
  );
  const canSave = $derived(busy === null && urlUsable && changed);
  const canTest = $derived(busy === null && urlUsable);
  const showSavedUrl = $derived(sameKind && remote.urlRedacted !== null);

  function edited(): void {
    status = null;
  }

  /** Trusting every script is a choice about one database: a new address or kind starts over, and the user turns it on again for the new one. */
  function databaseChanged(): void {
    trustTeamScripts = false;
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

  async function save(): Promise<void> {
    busy = 'save';
    status = null;

    try {
      await memoryRemote.save({
        enabled: true,
        kind,
        url: typedUrl === '' ? null : typedUrl,
        token: tokenToSend(),
        repositoryNotes,
        globalNotes,
        scripts,
        // The switch only shows while scripts live in the team database, so a hidden one never stays on.
        trustTeamScripts: scripts === 'remote' && trustTeamScripts,
      });

      // The secrets leave the form as soon as they are stored.
      url = '';
      token = '';
      touched = false;
      status = { ok: true, text: t('settings.memoryRemote.saved') };
    } catch (error) {
      status = { ok: false, text: failure(error) };
    } finally {
      busy = null;
    }
  }
</script>

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
        describedby={urlHint}
        onblur={() => (touched ||= typedUrl !== '')}
      />
      {#if urlInvalid && touched}
        <p id={urlHint} class="text-xs text-danger" role="alert">{t(URL_PROBLEMS[kind])}</p>
      {:else if showSavedUrl}
        <p id={urlHint} class="text-xs text-fg-3">{t('settings.memoryRemote.url.saved', { url: remote.urlRedacted ?? '' })}</p>
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

  <SettingRow title={t('settings.memoryRemote.repositoryNotes.title')} description={t('settings.memoryRemote.repositoryNotes.description')}>
    {#snippet control()}
      <SegmentedControl label={t('settings.memoryRemote.repositoryNotes.title')} options={TARGETS} bind:value={() => repositoryNotes, (next) => ((repositoryNotes = next), edited())} />
    {/snippet}
  </SettingRow>

  <SettingRow title={t('settings.memoryRemote.globalNotes.title')} description={t('settings.memoryRemote.globalNotes.description')}>
    {#snippet control()}
      <SegmentedControl label={t('settings.memoryRemote.globalNotes.title')} options={TARGETS} bind:value={() => globalNotes, (next) => ((globalNotes = next), edited())} />
    {/snippet}
  </SettingRow>

  <SettingRow
    title={t('settings.memoryRemote.scripts.title')}
    description={remote.scriptsSupported ? t('settings.memoryRemote.scripts.description') : t('settings.memoryRemote.scripts.unsupported')}
    descriptions={[t('settings.memoryRemote.scripts.description'), t('settings.memoryRemote.scripts.unsupported')]}
  >
    {#snippet control()}
      <SegmentedControl label={t('settings.memoryRemote.scripts.title')} options={SCRIPT_TARGETS} bind:value={() => scripts, (next) => ((scripts = next), edited())} />
    {/snippet}
  </SettingRow>

  <!-- Only scripts kept in the team database come from other people, so the question only exists while they are. -->
  {#if scripts === 'remote'}
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
