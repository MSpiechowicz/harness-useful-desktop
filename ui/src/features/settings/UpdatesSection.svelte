<script lang="ts">
  import Download from '@lucide/svelte/icons/download';
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

  <SettingRow title={t('settings.updates.policy.title')} description={POLICY_HINTS[policy]()} descriptions={Object.values(POLICY_HINTS).map((hint) => hint())}>
    {#snippet control()}
      <SegmentedControl label={t('settings.updates.policy.label')} options={POLICY_OPTIONS} bind:value={() => policy, (updatePolicy) => void prefs.update({ updatePolicy })} />
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
    <div class="flex items-start gap-3 py-4 text-sm text-fg-2 last:pb-0" role="status">
      <Info class="mt-0.5 shrink-0 text-fg-3" size={ICON.sm} aria-hidden="true" />

      <p class="min-w-0 flex-1">{t('settings.updates.notConfigured')}</p>

      <Button class="shrink-0" onclick={() => void openExternal(RELEASES_URL)}>
        {#snippet leading()}<ExternalLink size={ICON.sm} aria-hidden="true" />{/snippet}
        {t('settings.updates.openReleases')}
      </Button>
    </div>
  {/if}

  {#if failure}
    <div class="flex flex-col items-start gap-3 py-4 last:pb-0">
      <div class="w-full">
        <NoticeBanner notices={[{ level: failure.reason === 'verification' ? 'error' : 'warning', message: failureMessage(failure.reason, failure.during) }]} />
      </div>

      {#if canRetry}
        <Button onclick={() => void updates.install()}>{t('settings.updates.retry')}</Button>
      {/if}
    </div>
  {/if}

  {#if release && state.kind !== 'failed'}
    <div class="flex flex-col gap-4 py-4 last:pb-0">
      {#if date}<p class="text-sm text-fg-3">{date}</p>{/if}

      {#if notes}
        <!-- The notes are untrusted text from the release; only the sanitized Markdown component may show them. -->
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
        <div class="max-h-64 overflow-y-auto rounded-md border border-line bg-surface-2 p-4" role="region" aria-label={t('settings.updates.notes')} tabindex="0">
          <Markdown source={notes} />
        </div>
      {/if}

      <!-- The dashboard's compact update card: a single rounded surface with the available version and a one-click
           "Install & restart" button (whose download icon bobs so a waiting update is hard to miss). Mirrors the
           pattern in harness-useful-dashboard so the two apps feel like the same product. -->
      <div class="rounded-lg border border-line bg-accent-wash px-3 py-2.5 text-xs">
        <div class="font-medium text-fg">{t('updates.status.available', { version: release.version })}</div>

        {#if state.kind === 'available'}
          <div class="mt-2">
            <Button variant="primary" onclick={() => void updates.install()}>
              {#snippet leading()}<span class="update-arrow"><Download size={ICON.sm} aria-hidden="true" /></span>{/snippet}
              {t('updates.install.restart')}
            </Button>
          </div>
        {:else if state.kind === 'downloading' || state.kind === 'installing'}
          <div class="mt-2"><UpdateProgress /></div>
        {:else if state.kind === 'readyToRelaunch'}
          <div class="mt-2">
            <Button variant="primary" onclick={() => void updates.requestRelaunch()}>
              {#snippet leading()}<RotateCw size={ICON.sm} aria-hidden="true" />{/snippet}
              {updates.closesApp ? t('settings.updates.finish') : t('settings.updates.restart')}
            </Button>
          </div>
        {/if}
      </div>

      {#if updates.closesApp && ['available', 'downloading', 'installing', 'readyToRelaunch'].includes(state.kind)}
        <p class="text-sm text-fg-3">{installerClosesApp()}</p>
      {:else if state.kind === 'readyToRelaunch' && !updates.closesApp}
        <p class="text-sm text-fg-3">{restartStopsAgents()}</p>
      {/if}
    </div>
  {/if}
</SettingsSection>

<style>
  /* The download icon bobs on the "Install & restart" button so a waiting update is hard to miss. Still for whoever
     asked for less motion. */
  .update-arrow {
    display: inline-flex;
    animation: update-bob 1.8s ease-in-out infinite;
  }
  @keyframes update-bob {
    0%,
    100% {
      transform: translateY(-1px);
    }
    50% {
      transform: translateY(1.5px);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .update-arrow {
      animation: none;
    }
  }
</style>
