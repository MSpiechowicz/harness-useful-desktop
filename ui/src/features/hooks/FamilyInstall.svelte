<script lang="ts">
  import Download from '@lucide/svelte/icons/download';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import { ipc } from '$lib/ipc/client';
  import { IpcFailure } from '$lib/ipc/transport';
  import { ecosystem } from '$lib/stores/ecosystem.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';

  import { CompanionInstaller } from '../onboarding/companion-installer.svelte';
  import SetupConfirm from '../onboarding/SetupConfirm.svelte';
  import { type Companion, companionPlan, type Plan } from '../onboarding/setup-model';
  import { MEMBER_LABELS } from './hook-format';
  import { hooks } from './hooks-data.svelte';

  interface Props {
    member: Companion;
    /** Runs once the installer finished well. */
    onfinished?: () => void;
  }

  let { member, onfinished }: Props = $props();

  type Phase = 'idle' | 'reading' | 'running';

  // svelte-ignore state_referenced_locally
  const installer = new CompanionInstaller(member, ipc);

  const name = $derived(t(MEMBER_LABELS[member]));

  let phase = $state<Phase>('idle');
  let plan = $state.raw<Plan | null>(null);
  let confirming = $state(false);
  let failure = $state('');

  /** The installer's command and effects go on screen first. Only the user's confirmation starts it. */
  async function review(): Promise<void> {
    failure = '';
    phase = 'reading';

    try {
      plan = companionPlan(member, (await ipc.invoke('setup_status', undefined))[member]);
      confirming = true;
    } catch (error) {
      failure = t('hooks.family.readFailed', { reason: IpcFailure.from(error).message });
    }

    phase = 'idle';
  }

  async function run(): Promise<void> {
    phase = 'running';

    const outcome = await installer.run();

    phase = 'idle';

    if (!outcome.ok) {
      failure = t('onboarding.setup.companion.failed', { reason: outcome.failure || t('onboarding.setup.companion.noReason') });

      return;
    }

    // The Ecosystem page and the memory preferences follow the new tool, and the installer restores its missing family hooks.
    void ecosystem.load(true);
    void prefs.syncServerManaged().catch(() => {});
    await hooks.refresh();
    onfinished?.();
  }
</script>

<div class="flex flex-col items-start gap-2">
  <Button loading={phase !== 'idle'} onclick={() => void review()}>
    {#snippet leading()}<Download size={ICON.sm} aria-hidden="true" />{/snippet}
    {phase === 'running' ? t('hooks.family.installing', { member: name }) : t('hooks.family.install', { member: name })}
  </Button>

  {#if failure}<p class="text-sm text-danger" role="alert">{failure}</p>{/if}
</div>

{#if plan}
  <SetupConfirm bind:open={confirming} {plan} onconfirm={() => void run()} title={t('hooks.family.confirm.title', { member: name })} body={t('hooks.family.confirm.body')} confirmLabel={t('hooks.family.install', { member: name })} />
{/if}
