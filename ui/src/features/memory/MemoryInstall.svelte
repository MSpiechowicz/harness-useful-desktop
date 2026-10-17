<script lang="ts">
  import Download from '@lucide/svelte/icons/download';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';

  import Button from '$lib/components/Button.svelte';
  import Disclosure from '$lib/components/Disclosure.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { rise } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { ipc } from '$lib/ipc/client';
  import { IpcFailure } from '$lib/ipc/transport';
  import { ecosystem } from '$lib/stores/ecosystem.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';

  import { CompanionInstaller } from '../onboarding/companion-installer.svelte';
  import SetupConfirm from '../onboarding/SetupConfirm.svelte';
  import SetupLog from '../onboarding/SetupLog.svelte';
  import { companionPlan, type Plan } from '../onboarding/setup-model';
  import { memoryUpgradeAvailable } from './memory-upgrade';

  interface Props {
    /** Harness Memory is installed but too old to keep scripts. Otherwise it is not installed at all. */
    update: boolean;
    /** Reads the scripts again after an install, which finds out whether Harness Memory keeps them now. */
    onfinished: () => Promise<void>;
    /** One quiet button for an update that adds a feature to a view that works already (the progress and the failure stay in that button), instead of the actions of a view that cannot work at all. */
    inline?: boolean;
  }

  let { update, onfinished, inline = false }: Props = $props();

  type Phase = 'idle' | 'reading' | 'running' | 'failed';

  const installer = new CompanionInstaller('memory', ipc);

  let phase = $state<Phase>('idle');
  let plan = $state.raw<Plan | null>(null);
  let confirming = $state(false);
  let failure = $state('');
  let logOpen = $state(true);

  const label = $derived(update ? t('memory.install.update') : t('memory.install.install'));
  const progress = $derived(update ? t('memory.install.updating') : t('memory.install.installing'));

  /** Reads what the installer would do, then asks. Nothing runs before the user confirmed it. */
  async function ask(): Promise<void> {
    phase = 'reading';

    // A source build (`1.0.1-dev`) or the latest release may have nothing newer to update to: the installer would only put back the same or an older one.
    if (update && !(await memoryUpgradeAvailable())) {
      fail(t('memory.install.noRelease'));

      return;
    }

    try {
      const setup = await ipc.invoke('setup_status', undefined);
      plan = companionPlan('memory', setup.memory);
      phase = 'idle';
      confirming = true;
    } catch (error) {
      fail(t('memory.install.readFailed', { reason: IpcFailure.from(error).message }));
    }
  }

  async function run(): Promise<void> {
    phase = 'running';

    // The reading the button was offered on may be old: an update runs only while the latest release is still newer than what is installed.
    if (update && !(await memoryUpgradeAvailable())) {
      fail(t('memory.install.noRelease'));

      return;
    }

    const outcome = await installer.run();

    if (!outcome.ok) {
      fail(installer.result?.timedOut ? t('onboarding.setup.companion.timedOut') : t('onboarding.setup.companion.failed', { reason: outcome.failure || t('onboarding.setup.companion.noReason') }));

      return;
    }

    // The Ecosystem page and the memory preferences follow the new binary, like after any other install.
    void ecosystem.load(true);
    void prefs.syncServerManaged().catch(() => {});

    await onfinished();

    // Still here after the list was read again: it is still unavailable (or, inline, still without the feature), and the view shows nothing else about it.
    fail(t(inline ? 'memory.install.stillOld' : 'memory.install.stillUnavailable'));
  }

  function fail(reason: string): void {
    failure = reason;
    phase = 'failed';
  }
</script>

{#if inline}
  {#if phase === 'failed'}
    <p class="min-w-0 truncate text-sm text-danger" role="alert" title={failure}>{failure}</p>
  {/if}

  <Button variant="ghost" aria-label={label} loading={phase === 'reading' || phase === 'running'} onclick={() => void ask()}>
    {#snippet leading()}<RefreshCw size={16} aria-hidden="true" />{/snippet}
    {phase === 'running' ? progress : t('memory.filter.update')}
  </Button>
{:else if phase === 'running' || phase === 'failed'}
  <div in:rise={{ y: 8, duration: 280 }} class="flex w-full max-w-md flex-col gap-3 text-left">
    {#if phase === 'running'}
      <p class="flex items-center gap-2 text-sm text-fg-2" role="status">
        <Spinner size={16} />
        {progress}
      </p>
    {:else}
      <p class="text-sm text-danger" role="alert">{failure}</p>
    {/if}

    {#if installer.log.length > 0 || phase === 'running'}
      <Disclosure label={t('onboarding.setup.details')} bind:open={logOpen}>
        <SetupLog lines={installer.log} />
      </Disclosure>
    {/if}

    {#if phase === 'failed'}
      <div class="flex items-center justify-center gap-2">
        <Button variant="primary" onclick={() => void ask()}>{t('memory.tryAgain')}</Button>
        <Button variant="ghost" onclick={() => nav.go('ecosystem')}>{t('memory.unavailable.ecosystem')}</Button>
      </div>
    {/if}
  </div>
{:else}
  <div class="flex items-center justify-center gap-2">
    <Button variant="primary" loading={phase === 'reading'} onclick={() => void ask()}>
      {#snippet leading()}
        {#if update}<RefreshCw size={16} aria-hidden="true" />{:else}<Download size={16} aria-hidden="true" />{/if}
      {/snippet}
      {label}
    </Button>
    <Button variant="ghost" onclick={() => nav.go('ecosystem')}>{t('memory.unavailable.ecosystem')}</Button>
  </div>
{/if}

{#if plan}
  <SetupConfirm
    bind:open={confirming}
    {plan}
    onconfirm={() => void run()}
    title={update ? t('memory.install.confirm.update') : t('memory.install.confirm.install')}
    body={t('memory.install.confirm.body')}
    confirmLabel={label}
  />
{/if}
