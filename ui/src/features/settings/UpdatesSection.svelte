<script lang="ts">
  import ExternalLink from '@lucide/svelte/icons/external-link';
  import Info from '@lucide/svelte/icons/info';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import RotateCw from '@lucide/svelte/icons/rotate-cw';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import Markdown from '$lib/components/Markdown.svelte';
  import NoticeBanner from '$lib/components/NoticeBanner.svelte';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import Switch from '$lib/components/Switch.svelte';
  import type { StripOption } from '$lib/components/strip';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { UpdatePolicy } from '$lib/ipc/bindings/UpdatePolicy';
  import { openExternal } from '$lib/ipc/actions';
  import { app } from '$lib/stores/app.svelte';
  import { ecosystem } from '$lib/stores/ecosystem.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { retryable, updates } from '$lib/stores/updates.svelte';

  import { failureMessage, formatReleaseDate, installerClosesApp, releaseNotes, restartStopsAgents, statusLine } from '../updates/copy';
  import UpdateProgress from '../updates/UpdateProgress.svelte';
  import { RELEASES_URL } from './links';
  import SettingRow from './SettingRow.svelte';
  import SettingsSection from './SettingsSection.svelte';

  interface Props {
    id?: string;
  }

  let { id }: Props = $props();

  const POLICY_OPTIONS: StripOption<UpdatePolicy>[] = [
    {
      value: 'startupAsk',
      get label() {
        return t('settings.updates.policy.startupAsk');
      },
    },
    {
      value: 'autoInstall',
      get label() {
        return t('settings.updates.policy.autoInstall');
      },
    },
    {
      value: 'off',
      get label() {
        return t('settings.updates.policy.off');
      },
    },
  ];
  const POLICY_HINTS: Record<UpdatePolicy, () => string> = {
    startupAsk: () => t('settings.updates.policy.startupAsk.hint'),
    autoInstall: () => t('settings.updates.policy.autoInstall.hint'),
    off: () => t('settings.updates.policy.off.hint'),
  };

  const state = $derived(updates.state);
  const policy = $derived(prefs.value?.updatePolicy ?? 'startupAsk');
  const release = $derived(updates.info);
  const date = $derived(release ? formatReleaseDate(release.date) : null);
  const notes = $derived(release ? releaseNotes(release) : '');
  // A build without updates can only fail again, and after an install the old version would find the same update.
  const canCheck = $derived(state.kind !== 'notConfigured' && state.kind !== 'readyToRelaunch');
  const failure = $derived(state.kind === 'failed' ? state : null);
  const canRetry = $derived(failure !== null && failure.during === 'install' && retryable(failure.reason));
</script>

<SettingsSection {id} title={t('settings.section.updates')}>
  <SettingRow title={t('settings.updates.version', { version: app.info?.version ?? '' })} description={statusLine(state)}>
    {#snippet control()}
      {#if canCheck}
        <Button disabled={updates.busy} loading={state.kind === 'checking'} onclick={() => void updates.check()}>
          {#snippet leading()}<RefreshCw size={ICON.sm} aria-hidden="true" />{/snippet}
          {t('settings.updates.check')}
        </Button>
      {/if}
    {/snippet}
  </SettingRow>

  <SettingRow title={t('settings.updates.policy.title')} description={POLICY_HINTS[policy]()}>
    {#snippet control()}
      <SegmentedControl size="md" label={t('settings.updates.policy.label')} options={POLICY_OPTIONS} bind:value={() => policy, (updatePolicy) => void prefs.update({ updatePolicy })} />
    {/snippet}
  </SettingRow>

  <!-- The companions' own lookup (ecosystem_releases): separate from the app's updates, and off means no request at all. -->
  <SettingRow
    title={t('settings.updates.companions.title')}
    description={t('settings.updates.companions.description')}
  >
    {#snippet control()}
      <Switch
        label={t('settings.updates.companions.title')}
        bind:checked={
          () => prefs.value?.checkCompanionUpdates ?? true,
          // Forget the last reading, so Ecosystem shows (or drops) the GitHub badges on its next visit, not 30 s later.
          (checkCompanionUpdates) => void prefs.update({ checkCompanionUpdates }).then(() => ecosystem.reset())
        }
      />
    {/snippet}
  </SettingRow>

  {#if state.kind === 'notConfigured'}
    <div class="flex items-start gap-3 py-4 text-sm text-fg-2" role="status">
      <Info class="mt-0.5 shrink-0 text-fg-3" size={ICON.sm} aria-hidden="true" />

      <p class="min-w-0 flex-1">{t('settings.updates.notConfigured')}</p>

      <Button class="shrink-0" onclick={() => void openExternal(RELEASES_URL)}>
        {#snippet leading()}<ExternalLink size={ICON.sm} aria-hidden="true" />{/snippet}
        {t('settings.updates.openReleases')}
      </Button>
    </div>
  {/if}

  {#if failure}
    <div class="flex flex-col items-start gap-3 py-4">
      <div class="w-full">
        <NoticeBanner notices={[{ level: failure.reason === 'verification' ? 'error' : 'warning', message: failureMessage(failure.reason, failure.during) }]} />
      </div>

      {#if canRetry}
        <Button onclick={() => void updates.install()}>{t('settings.updates.retry')}</Button>
      {/if}
    </div>
  {/if}

  {#if release && state.kind !== 'failed'}
    <div class="flex flex-col gap-4 py-4">
      <div class="flex items-baseline justify-between gap-4">
        <p class="text-base font-medium text-fg">{t('settings.updates.version', { version: release.version })}</p>
        {#if date}<p class="text-sm text-fg-3">{date}</p>{/if}
      </div>

      {#if notes}
        <!-- The notes are untrusted text from the release; only the sanitized Markdown component may show them. -->
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
        <div class="max-h-64 overflow-y-auto rounded-md border border-line bg-surface-2 p-4" role="region" aria-label={t('settings.updates.notes')} tabindex="0">
          <Markdown source={notes} />
        </div>
      {/if}

      {#if state.kind === 'available'}
        <div class="flex flex-wrap items-center gap-3">
          <Button variant="primary" onclick={() => void updates.install()}>{t('settings.updates.install')}</Button>
          {#if updates.closesApp}<p class="text-sm text-fg-3">{installerClosesApp()}</p>{/if}
        </div>
      {:else if state.kind === 'downloading' || state.kind === 'installing'}
        <UpdateProgress />
        {#if updates.closesApp}<p class="text-sm text-fg-3">{installerClosesApp()}</p>{/if}
      {:else if state.kind === 'readyToRelaunch'}
        <div class="flex flex-wrap items-center gap-3">
          <Button variant="primary" onclick={() => void updates.requestRelaunch()}>
            {#snippet leading()}<RotateCw size={ICON.sm} aria-hidden="true" />{/snippet}
            {updates.closesApp ? t('settings.updates.finish') : t('settings.updates.restart')}
          </Button>
          <p class="text-sm text-fg-3">{updates.closesApp ? installerClosesApp() : restartStopsAgents()}</p>
        </div>
      {/if}
    </div>
  {/if}
</SettingsSection>
