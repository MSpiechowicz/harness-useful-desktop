<script lang="ts">
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { cx } from '$lib/utils/cx';

  import HarnessIntroCard from './HarnessIntroCard.svelte';
  import HarnessScan from './HarnessScan.svelte';
  import { HARNESS_GRID, WIDTH } from './layout';
  import StepHeader from './StepHeader.svelte';
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
