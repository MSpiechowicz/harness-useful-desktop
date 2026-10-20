<script lang="ts">
  import { untrack } from 'svelte';

  import Skeleton from '$lib/components/Skeleton.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';

  import HarnessIntroCard from './HarnessIntroCard.svelte';
  import HarnessScan from './HarnessScan.svelte';
  import { HARNESS_GRID, WIDTH } from './layout';
  import { machineFacts, projectToCount } from './machine-facts.svelte';
  import StepHeader from './StepHeader.svelte';

  // The picker comes next and shows what each installed harness has on this machine: reading it now means it is usually there already.
  $effect(() => {
    const target = projectToCount(project.active, project.recents);
    const summaries = harness.summaries;

    untrack(() => machineFacts.prefetch(summaries, target));
  });
</script>

<div class={cx('mx-auto flex w-full flex-col gap-8 [@media(max-height:720px)]:gap-4', WIDTH.wide)}>
  <StepHeader eyebrow={t('onboarding.meet.eyebrow')} title={t('onboarding.meet.title')} body={t('onboarding.meet.body')} />

  <HarnessScan>
    {#snippet loading()}
      <div class={HARNESS_GRID}>
        <Skeleton height="12rem" />
        <Skeleton height="12rem" />
        <Skeleton height="12rem" />
      </div>
    {/snippet}

    <div class={HARNESS_GRID}>
      {#each harness.summaries as summary (summary.info.id)}
        <HarnessIntroCard {summary} />
      {/each}
    </div>
  </HarnessScan>
</div>
