<script lang="ts">
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { DashboardSetup } from '$lib/ipc/bindings/DashboardSetup';
  import Button from '$lib/components/Button.svelte';

  import SetupCard from './SetupCard.svelte';
  import SetupCommand from './SetupCommand.svelte';
  import SetupEffects from './SetupEffects.svelte';
  import SetupLog from './SetupLog.svelte';
  import { dashboardChip } from './setup-model';
  import { setup } from './setup-state.svelte';

  interface Props {
    dashboard: DashboardSetup;
  }

  let { dashboard }: Props = $props();

  const phase = $derived(setup.phase('dashboard'));
  const showActivity = $derived(phase !== 'idle' || setup.log.length > 0);
</script>

<!-- The card is its own row: nothing repeats what the header already says. -->
<SetupCard
  product="dashboard"
  title={t('onboarding.setup.dashboard.title')}
  chip={dashboardChip(dashboard, setup.shown('dashboard'), setup.waiting('dashboard'))}
  description={t('onboarding.setup.dashboard.body')}
  detailsMeta={dashboard.supportedOnThisOs ? tn('onboarding.setup.dashboard.changes', dashboard.sideEffects.length) : undefined}
>
  {#snippet activity()}
    {#if showActivity}
      <div class="flex flex-col gap-2 border-t border-line pt-2.5">
        <p class="text-xs font-medium tracking-wider text-fg-3 uppercase">{t('onboarding.setup.log')}</p>
        <SetupLog lines={setup.log} />

        {#if phase === 'failed'}
          <div class="flex items-center gap-3" role="alert">
            <p class="min-w-0 flex-1 text-sm text-danger">{setup.installResult?.timedOut ? t('onboarding.setup.dashboard.timedOut') : t('onboarding.setup.dashboard.failed', { reason: setup.failure('dashboard') || t('onboarding.setup.dashboard.noReason') })}</p>
            <Button disabled={setup.running} onclick={() => setup.ask()}>{t('onboarding.setup.retry')}</Button>
          </div>
        {/if}
      </div>
    {/if}
  {/snippet}

  {#snippet details()}
    {#if dashboard.supportedOnThisOs}
      <section class="flex flex-col gap-2" aria-label={t('onboarding.setup.dashboard.command')}>
        <h4 class="text-xs font-medium tracking-wider text-fg-3 uppercase">{t('onboarding.setup.dashboard.command')}</h4>
        <p>{t('onboarding.setup.dashboard.rowDetail')}</p>
        <SetupCommand text={dashboard.command} copyLabel={t('onboarding.setup.copyCommand')} />
      </section>

      <section class="flex flex-col gap-3" aria-label={t('onboarding.setup.dashboard.effects')}>
        <h4 class="text-xs font-medium tracking-wider text-fg-3 uppercase">{t('onboarding.setup.dashboard.effects')}</h4>
        <SetupEffects effects={dashboard.sideEffects} />
      </section>
    {:else}
      <p>{t('onboarding.setup.dashboard.unsupported')}</p>
    {/if}
  {/snippet}
</SetupCard>
