<script lang="ts">
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { HarnessSetup } from '$lib/ipc/bindings/HarnessSetup';
  import { cx } from '$lib/utils/cx';

  import SetupCard from './SetupCard.svelte';
  import SetupChip from './SetupChip.svelte';
  import SetupCommand from './SetupCommand.svelte';
  import SetupEffects from './SetupEffects.svelte';
  import { commandOf, skillsChip, skillsOverviewChip, unavailableReason } from './setup-model';
  import { badgeOf, nameOf } from './setup-view';

  interface Props {
    /** One entry per harness the backend lists; the card says what the skills setup does in each. */
    harnesses: HarnessSetup[];
  }

  let { harnesses }: Props = $props();

  /** The commands the setup would run for the skills, across every harness that lacks them. A harness with its own installer counts one per change it lists. */
  const changes = $derived(
    harnesses.reduce((count, entry) => {
      if (!entry.installed) {
        return count;
      }

      if (entry.skills.type === 'install') {
        return count + entry.skills.actions.length;
      }

      return entry.skills.type === 'script' ? count + entry.skills.sideEffects.length : count;
    }, 0),
  );
</script>

<!-- Built like the two installer cards above it: header line, description, details. -->
<SetupCard
  product="skills"
  title={t('onboarding.setup.skills.title')}
  chip={skillsOverviewChip(harnesses)}
  description={t('onboarding.setup.skills.body')}
  detailsMeta={changes > 0 ? tn('onboarding.setup.companion.changes', changes) : undefined}
>
  {#snippet details()}
    <section class="flex flex-col gap-3" aria-label={t('onboarding.setup.skills.perHarness')}>
      <h4 class="text-xs font-medium tracking-wider text-fg-3 uppercase">{t('onboarding.setup.skills.perHarness')}</h4>

      <ul class="flex flex-col gap-4">
        {#each harnesses as entry (entry.harness)}
          {@const name = nameOf(entry.harness)}
          <li class={cx('flex flex-col gap-2', !entry.installed && 'opacity-60')}>
            <div class="flex items-center justify-between gap-3">
              <div class="flex min-w-0 items-center gap-2.5">
                <HarnessBadge info={badgeOf(entry.harness)} size="tile" />
                <p class="truncate text-sm font-medium text-fg" title={name}>{name}</p>
              </div>

              <SetupChip chip={skillsChip(entry, 'idle', false)} />
            </div>

            {#if !entry.installed}
              <p class="text-fg-3">{t('onboarding.setup.skills.installFirst', { name })}</p>
            {:else if entry.skills.type === 'installed'}
              {#if entry.skills.version}
                <p class="text-fg-3">{t('onboarding.setup.skills.version', { version: entry.skills.version })}</p>
              {/if}
            {:else if entry.skills.type === 'install'}
              {#each entry.skills.actions as action, index (index)}
                <SetupCommand text={commandOf(entry.harness, action)} copyLabel={t('onboarding.setup.copyCommand')} />
              {/each}
            {:else if entry.skills.type === 'script'}
              <SetupCommand text={entry.skills.command} copyLabel={t('onboarding.setup.copyCommand')} />
              <SetupEffects effects={entry.skills.sideEffects} />
            {:else if entry.skills.type === 'manual'}
              <p>{entry.skills.note}</p>
              <SetupCommand text={entry.skills.command} copyLabel={t('onboarding.setup.copyCommand')} />
            {:else}
              <p>{unavailableReason(entry.skills)}</p>
            {/if}
          </li>
        {/each}
      </ul>
    </section>

    <p class="text-fg-3">{t('onboarding.setup.skills.note')}</p>
  {/snippet}
</SetupCard>
