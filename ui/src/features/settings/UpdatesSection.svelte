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
  import type { StripOption } from '$lib/components/strip';
  import type { UpdatePolicy } from '$lib/ipc/bindings/UpdatePolicy';
  import { openExternal } from '$lib/ipc/actions';
  import { app } from '$lib/stores/app.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { retryable, updates } from '$lib/stores/updates.svelte';

  import { failureMessage, formatReleaseDate, INSTALLER_CLOSES_APP, releaseNotes, RESTART_STOPS_AGENTS, statusLine } from '../updates/copy';
  import UpdateProgress from '../updates/UpdateProgress.svelte';
  import { RELEASES_URL } from './links';
  import SettingRow from './SettingRow.svelte';
  import SettingsSection from './SettingsSection.svelte';

  interface Props {
    id?: string;
  }

  let { id }: Props = $props();

  const POLICY_OPTIONS: StripOption<UpdatePolicy>[] = [
    { value: 'startupAsk', label: 'Ask first' },
    { value: 'autoInstall', label: 'Automatic' },
    { value: 'off', label: 'Off' },
  ];
  const POLICY_HINTS: Record<UpdatePolicy, string> = {
    startupAsk: 'Looks for a new version when the app starts and asks before installing it.',
    autoInstall: 'Looks when the app starts and installs a new version. You choose when to restart.',
    off: 'Never looks by itself. Use Check now when you want to.',
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

<SettingsSection {id} title="Updates">
  <SettingRow title="Version {app.info?.version ?? ''}" description={statusLine(state)}>
    {#snippet control()}
      {#if canCheck}
        <Button disabled={updates.busy} loading={state.kind === 'checking'} onclick={() => void updates.check()}>
          {#snippet leading()}<RefreshCw size={ICON.sm} aria-hidden="true" />{/snippet}
          Check now
        </Button>
      {/if}
    {/snippet}
  </SettingRow>

  <SettingRow title="When the app starts" description={POLICY_HINTS[policy]}>
    {#snippet control()}
      <SegmentedControl label="Update policy" options={POLICY_OPTIONS} bind:value={() => policy, (updatePolicy) => void prefs.update({ updatePolicy })} />
    {/snippet}
  </SettingRow>

  {#if state.kind === 'notConfigured'}
    <div class="flex items-start gap-3 py-4 text-sm text-fg-2" role="status">
      <Info class="mt-0.5 shrink-0 text-fg-3" size={ICON.sm} aria-hidden="true" />

      <p class="min-w-0 flex-1">This build is not set up for updates, so it will not look for new versions. Nothing is wrong. Get new versions from the releases page.</p>

      <Button size="sm" class="shrink-0" onclick={() => void openExternal(RELEASES_URL)}>
        {#snippet leading()}<ExternalLink size={ICON.sm} aria-hidden="true" />{/snippet}
        Open releases
      </Button>
    </div>
  {/if}

  {#if failure}
    <div class="flex flex-col items-start gap-3 py-4">
      <div class="w-full">
        <NoticeBanner notices={[{ level: failure.reason === 'verification' ? 'error' : 'warning', message: failureMessage(failure.reason, failure.during) }]} />
      </div>

      {#if canRetry}
        <Button onclick={() => void updates.install()}>Try again</Button>
      {/if}
    </div>
  {/if}

  {#if release && state.kind !== 'failed'}
    <div class="flex flex-col gap-4 py-4">
      <div class="flex items-baseline justify-between gap-4">
        <p class="text-base font-medium text-fg">Version {release.version}</p>
        {#if date}<p class="text-sm text-fg-3">{date}</p>{/if}
      </div>

      {#if notes}
        <!-- The notes are untrusted text from the release; only the sanitized Markdown component may show them. -->
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
        <div class="max-h-64 overflow-y-auto rounded-md border border-line bg-surface-2 p-4" role="region" aria-label="Release notes" tabindex="0">
          <Markdown source={notes} />
        </div>
      {/if}

      {#if state.kind === 'available'}
        <div class="flex flex-wrap items-center gap-3">
          <Button variant="primary" onclick={() => void updates.install()}>Install update</Button>
          {#if updates.closesApp}<p class="text-sm text-fg-3">{INSTALLER_CLOSES_APP}</p>{/if}
        </div>
      {:else if state.kind === 'downloading' || state.kind === 'installing'}
        <UpdateProgress />
        {#if updates.closesApp}<p class="text-sm text-fg-3">{INSTALLER_CLOSES_APP}</p>{/if}
      {:else if state.kind === 'readyToRelaunch'}
        <div class="flex flex-wrap items-center gap-3">
          <Button variant="primary" onclick={() => void updates.requestRelaunch()}>
            {#snippet leading()}<RotateCw size={ICON.sm} aria-hidden="true" />{/snippet}
            {updates.closesApp ? 'Finish update' : 'Restart to finish'}
          </Button>
          <p class="text-sm text-fg-3">{updates.closesApp ? INSTALLER_CLOSES_APP : RESTART_STOPS_AGENTS}</p>
        </div>
      {/if}
    </div>
  {/if}
</SettingsSection>
