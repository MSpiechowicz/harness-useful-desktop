<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { RecommendedSetup } from '$lib/ipc/bindings/RecommendedSetup';
  import { joinNames } from '$lib/utils/format';
  import { uid } from '$lib/utils/ids';

  import SetupCommand from './SetupCommand.svelte';
  import SetupFamilyRow from './SetupFamilyRow.svelte';
  import SetupLog from './SetupLog.svelte';
  import { companionChip, skillsRowChip, skillsTargets, type Companion } from './setup-model';
  import { setup } from './setup-state.svelte';
  import { nameOf } from './setup-view';

  interface Props {
    data: RecommendedSetup;
  }

  let { data }: Props = $props();

  const titleId = uid('setup-family');

  /** The two installers, in the order they run; the skills follow them. */
  const COMPANIONS: Companion[] = ['dashboard', 'memory'];

  const NAMES: Record<Companion, () => string> = {
    dashboard: () => t('onboarding.setup.dashboard.title'),
    memory: () => t('onboarding.setup.memory.title'),
  };

  const targets = $derived(skillsTargets(data.harnesses));
  const skillsDetail = $derived(targets.length > 0 ? t('onboarding.setup.skills.rowDetail', { harnesses: joinNames(targets.map((entry) => nameOf(entry.harness))) }) : t('onboarding.setup.skills.rowNone'));

  /** Harnesses whose skills the app cannot install but the user can, with a command to run by hand. */
  const manual = $derived(data.harnesses.filter((entry) => entry.installed && entry.skills.type === 'manual'));
  const failures = $derived(setup.skillsFailures());
</script>

<!-- The whole Harness family on the step that sets it up: one row each, whatever its state, in the order they run, and the output under it once there is some. -->
<section aria-labelledby={titleId} class="flex min-w-0 flex-col gap-1 rounded-lg border border-line bg-glass p-4 shadow-float [@media(max-height:720px)]:p-3.5">
  <h3 id={titleId} class="truncate text-xs font-medium tracking-wider text-fg-3 uppercase">{t('onboarding.setup.installers.title')}</h3>

  <ul class="mt-2 flex flex-col">
    {#each COMPANIONS as companion (companion)}
      {@const server = data[companion]}
      {@const installer = setup.installers[companion]}
      {@const phase = setup.phase(companion)}

      <SetupFamilyRow
        product={companion}
        name={NAMES[companion]()}
        detail={server.supportedOnThisOs ? t('onboarding.setup.companion.rowDetail') : t('onboarding.setup.companion.unsupported')}
        chip={companionChip(server, setup.shown(companion), setup.waiting(companion))}
      >
        {#if phase !== 'idle' || installer.log.length > 0}
          <div class="flex flex-col gap-2">
            <SetupLog lines={installer.log} />

            {#if phase === 'failed'}
              {@render failure([installer.result?.timedOut ? t('onboarding.setup.companion.timedOut') : t('onboarding.setup.companion.failed', { reason: setup.failure(companion) || t('onboarding.setup.companion.noReason') })])}
            {/if}
          </div>
        {/if}
      </SetupFamilyRow>
    {/each}

    <SetupFamilyRow product="skills" name={t('onboarding.setup.skills.title')} detail={skillsDetail} chip={skillsRowChip(data.harnesses, setup.skillsShown(), setup.skillsWaiting())}>
      {#if failures.length > 0 || manual.length > 0}
        <div class="flex flex-col gap-2">
          {#if failures.length > 0}
            {@render failure(failures.map(({ harness, reason }) => t('onboarding.setup.skills.failedIn', { name: nameOf(harness), reason })))}
          {/if}

          {#each manual as entry (entry.harness)}
            {@const name = nameOf(entry.harness)}

            {#if entry.skills.type === 'manual'}
              <section class="flex flex-col gap-2" aria-label={name}>
                <h4 class="text-xs font-medium tracking-wider text-fg-3 uppercase">{name}</h4>
                <p class="text-sm text-fg-2">{entry.skills.note}</p>
                <SetupCommand text={entry.skills.command} copyLabel={t('onboarding.setup.copyCommand')} />
              </section>
            {/if}
          {/each}

        </div>
      {/if}
    </SetupFamilyRow>
  </ul>
</section>

{#snippet failure(messages: string[])}
  <div class="flex items-center gap-3" role="alert">
    <div class="flex min-w-0 flex-1 flex-col gap-1">
      {#each messages as message, index (index)}
        <p class="text-sm text-danger">{message}</p>
      {/each}
    </div>
    <Button disabled={setup.running} onclick={() => setup.ask()}>{t('onboarding.setup.retry')}</Button>
  </div>
{/snippet}
