<script lang="ts">
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { DashboardSetup } from '$lib/ipc/bindings/DashboardSetup';

  import SetupCard from './SetupCard.svelte';
  import SetupCommand from './SetupCommand.svelte';
  import SetupEffects from './SetupEffects.svelte';
  import { companionChip, companionEffects, type Companion } from './setup-model';
  import { setup } from './setup-state.svelte';

  interface Props {
    /** Which family member the card is for; both cards are built the same way, in every state. */
    companion: Companion;
    server: DashboardSetup;
  }

  let { companion, server }: Props = $props();

  const effects = $derived(companionEffects(companion, server));
</script>

<!-- The run itself, with its output, is shown on the next step, beside the rest of what gets installed. -->
<SetupCard
  product={companion}
  title={companion === 'memory' ? t('onboarding.setup.memory.title') : t('onboarding.setup.dashboard.title')}
  chip={companionChip(server, setup.shown(companion), setup.waiting(companion))}
  description={companion === 'memory' ? t('onboarding.setup.memory.body') : t('onboarding.setup.dashboard.body')}
  detailsMeta={server.supportedOnThisOs ? tn('onboarding.setup.companion.changes', effects.length) : undefined}
>
  {#snippet details()}
    {#if server.supportedOnThisOs}
      <section class="flex flex-col gap-2" aria-label={t('onboarding.setup.companion.command')}>
        <h4 class="text-xs font-medium tracking-wider text-fg-3 uppercase">{t('onboarding.setup.companion.command')}</h4>
        <p>{t('onboarding.setup.companion.rowDetail')}</p>
        <SetupCommand text={server.command} copyLabel={t('onboarding.setup.copyCommand')} />
      </section>

      <section class="flex flex-col gap-3" aria-label={t('onboarding.setup.companion.effects')}>
        <h4 class="text-xs font-medium tracking-wider text-fg-3 uppercase">{t('onboarding.setup.companion.effects')}</h4>
        <SetupEffects {effects} />
      </section>
    {:else}
      <p>{t('onboarding.setup.companion.unsupported')}</p>
    {/if}
  {/snippet}
</SetupCard>
