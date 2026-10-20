<script lang="ts">
  import { untrack } from 'svelte';

  import Skeleton from '$lib/components/Skeleton.svelte';
  import StatusDot from '$lib/components/StatusDot.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  import HarnessDetail from './HarnessDetail.svelte';
  import HarnessList from './HarnessList.svelte';
  import HarnessScan from './HarnessScan.svelte';
  import { PICKER, WIDTH } from './layout';
  import { machineFacts, projectToCount, readable } from './machine-facts.svelte';
  import { onboarding } from './onboarding-state.svelte';
  import StepHeader from './StepHeader.svelte';

  const installed = $derived(harness.installed);
  const panel = uid('harness-detail');

  /** The harness whose details are open: where the step opened, then wherever the user looked last. */
  let focusId = $state<HarnessId | null>(null);

  /** Where the step opens: on the daily driver, else the first harness found, else the first one missing. */
  const opening = $derived(onboarding.driver ?? installed[0]?.info.id ?? harness.summaries[0]?.info.id ?? null);

  // The opening choice is made once, so a harness turning up in a scan later does not pull the user away from the one they are reading.
  $effect(() => {
    if (focusId === null && opening !== null) {
      focusId = opening;
    }
  });

  const shown = $derived(harness.summaries.find((summary) => summary.info.id === (focusId ?? opening)));

  // The intro step already reads every harness. This covers a step opened without it (a re-run from here) and a harness found later: a harness is read once per run.
  $effect(() => {
    if (shown && readable(shown)) {
      const target = projectToCount(project.active, project.recents);

      untrack(() => machineFacts.load(shown, target));
    }
  });
</script>

<div class={cx('mx-auto flex w-full flex-col gap-8 [@media(max-height:720px)]:gap-4', WIDTH.wide)}>
  <StepHeader
    eyebrow={t('onboarding.harnesses.eyebrow')}
    title={t('onboarding.harnesses.title')}
    body={t('onboarding.harnesses.body')}
  />

  <HarnessScan>
    {#snippet loading()}
      <div class={PICKER.frame}>
        <div class={PICKER.columns}>
          <Skeleton height="12rem" />
          <Skeleton height="20rem" />
        </div>
      </div>
    {/snippet}

    <!-- The strip and the picker belong together: the picker's own gap apart, not the step's. -->
    <div class="flex flex-col gap-4">
      <!-- What the scan found, in a strip as wide as the picker below it. -->
      <div class="flex items-center gap-3 rounded-lg border border-line bg-glass px-4 py-2">
        <p class="flex min-w-0 items-center gap-2.5 text-sm text-fg-2" role="status">
          <StatusDot tone={installed.length === 0 ? 'warning' : installed.length === harness.summaries.length ? 'success' : 'accent'} beforeText />
          {#if installed.length === 0}
            {t('onboarding.harnesses.none')}
          {:else}
            {t('onboarding.harnesses.found', { found: installed.length, total: harness.summaries.length })}
          {/if}
        </p>
      </div>

      {#if shown}
        <div class={PICKER.frame}>
          <div class={PICKER.columns}>
            <div class="self-start">
              <HarnessList summaries={harness.summaries} focused={shown.info.id} driver={onboarding.driver} {panel} onfocus={(id) => (focusId = id)} />
            </div>

            <HarnessDetail id={panel} summary={shown} facts={machineFacts.of(shown.info.id)} selected={onboarding.driver === shown.info.id} onselect={() => onboarding.choose(shown.info.id)} />
          </div>
        </div>
      {/if}
    </div>
  </HarnessScan>
</div>
