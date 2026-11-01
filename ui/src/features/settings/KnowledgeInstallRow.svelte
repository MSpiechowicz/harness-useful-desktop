<script lang="ts">
  import Download from '@lucide/svelte/icons/download';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';

  import Button from '$lib/components/Button.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { ipc } from '$lib/ipc/client';
  import { IpcFailure } from '$lib/ipc/transport';
  import { ecosystem } from '$lib/stores/ecosystem.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';

  import { CompanionInstaller } from '../onboarding/companion-installer.svelte';
  import SetupConfirm from '../onboarding/SetupConfirm.svelte';
  import { companionPlan, type Plan } from '../onboarding/setup-model';
  import SettingRow from './SettingRow.svelte';

  interface Props {
    /** Harness Knowledge is installed but too old for what the app needs. Otherwise it is not installed at all. */
    update: boolean;
    /** Looks at the installed Harness Knowledge again once the installer ran. */
    onfinished: () => Promise<void>;
  }

  let { update, onfinished }: Props = $props();

  type Phase = 'idle' | 'reading' | 'running' | 'failed';

  const installer = new CompanionInstaller('knowledge', ipc);

  let phase = $state<Phase>('idle');
  let plan = $state.raw<Plan | null>(null);
  let confirming = $state(false);
  let failure = $state('');

  const Icon = $derived(update ? RefreshCw : Download);
  const label = $derived(update ? t('knowledge.settings.install.update') : t('knowledge.settings.install.install'));

  /** Reads what the installer would do, then asks. Nothing runs before the user confirmed it. */
  async function review(): Promise<void> {
    phase = 'reading';

    try {
      plan = companionPlan('knowledge', (await ipc.invoke('setup_status', undefined)).knowledge);
      phase = 'idle';
      confirming = true;
    } catch (error) {
      giveUp(t('knowledge.settings.install.readFailed', { reason: IpcFailure.from(error).message }));
    }
  }

  async function run(): Promise<void> {
    phase = 'running';

    const outcome = await installer.run();

    if (!outcome.ok) {
      giveUp(installer.result?.timedOut ? t('onboarding.setup.companion.timedOut') : t('onboarding.setup.companion.failed', { reason: outcome.failure || t('onboarding.setup.companion.noReason') }));

      return;
    }

    // The Ecosystem page and the knowledge preferences follow the new binary, like after any other install.
    void ecosystem.load(true);
    void prefs.syncServerManaged().catch(() => {});

    await onfinished();
    phase = 'idle';
  }

  function giveUp(reason: string): void {
    failure = reason;
    phase = 'failed';
  }
</script>

<SettingRow title={update ? t('knowledge.settings.install.outdated.title') : t('knowledge.settings.install.missing.title')} description={update ? t('knowledge.settings.install.outdated.description') : t('knowledge.settings.install.missing.description')}>
  {#snippet control()}
    <div class="flex items-center gap-2">
      <Button variant="ghost" onclick={() => nav.go('ecosystem')}>{t('knowledge.settings.install.ecosystem')}</Button>
      <Button variant="primary" loading={phase === 'reading' || phase === 'running'} onclick={() => void review()}>
        {#snippet leading()}
          <Icon size={16} aria-hidden="true" />
        {/snippet}
        {phase === 'running' ? t(update ? 'knowledge.settings.install.updating' : 'knowledge.settings.install.installing') : label}
      </Button>
    </div>
  {/snippet}
  {#snippet below()}
    {#if phase === 'failed'}
      <p class="pt-2 text-sm text-danger" role="alert">{failure}</p>
    {/if}
  {/snippet}
</SettingRow>

{#if plan}
  <SetupConfirm bind:open={confirming} {plan} onconfirm={() => void run()} title={update ? t('knowledge.settings.install.confirm.update') : t('knowledge.settings.install.confirm.install')} body={t('knowledge.settings.install.confirm.body')} confirmLabel={label} />
{/if}
