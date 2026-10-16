<script lang="ts">
  import RotateCw from '@lucide/svelte/icons/rotate-cw';

  import Button from '$lib/components/Button.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import StatusDot from '$lib/components/StatusDot.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { cx } from '$lib/utils/cx';
  import { rovingIndex } from '$lib/utils/roving';

  import HarnessCard from './HarnessCard.svelte';
  import { WIDTH } from './layout';
  import { onboarding } from './onboarding-state.svelte';
  import StepHeader from './StepHeader.svelte';

  const installed = $derived(harness.installed);

  /**
   * As many cards across as fit at a readable width: three on the full frame, two when there are two harnesses (their
   * empty third column folds away), fewer as the window narrows. The cards share its rows (see HarnessCard).
   */
  const CARD_GRID = 'grid grid-cols-[repeat(auto-fit,minmax(16rem,1fr))] gap-x-4 gap-y-4';

  // Arrow keys move the daily-driver choice between installed harnesses, like a native radio group.
  function onKeydown(event: KeyboardEvent): void {
    const current = installed.findIndex((summary) => summary.info.id === onboarding.driver);
    const next = rovingIndex(event.key, current, installed.length, 'vertical') ?? rovingIndex(event.key, current, installed.length, 'horizontal');
    if (next === null || installed.length === 0) {
      return;
    }

    event.preventDefault();
    onboarding.choose(installed[next].info.id);
  }
</script>

<div class={cx('mx-auto flex w-full flex-col gap-8 [@media(max-height:720px)]:gap-4', WIDTH.wide)}>
  <StepHeader
    eyebrow={t('onboarding.harnesses.eyebrow')}
    title={t('onboarding.harnesses.title')}
    body={t('onboarding.harnesses.body')}
  />

  {#if harness.status === 'loading' || harness.status === 'idle'}
    <div class={CARD_GRID} role="status" aria-label={t('onboarding.harnesses.looking')}>
      <Skeleton height="16rem" />
      <Skeleton height="16rem" />
      <Skeleton height="16rem" />
    </div>
  {:else if harness.status === 'error'}
    <p class="rounded-md border border-line bg-danger-wash p-4 text-sm text-fg" role="alert">
      {t('onboarding.harnesses.error', { message: harness.error?.message })}
    </p>
  {:else}
    <!-- What the scan found, in a strip as wide as the cards below it, with the way to scan again at its end. -->
    <div class="-mb-1.5 flex items-center justify-between gap-3 rounded-lg border border-line bg-glass px-4 py-2">
      <p class="flex min-w-0 items-center gap-2.5 text-sm text-fg-2" role="status">
        <StatusDot tone={installed.length === 0 ? 'warning' : installed.length === harness.summaries.length ? 'success' : 'accent'} />
        {#if installed.length === 0}
          {t('onboarding.harnesses.none')}
        {:else}
          {t('onboarding.harnesses.found', { found: installed.length, total: harness.summaries.length })}
        {/if}
      </p>
      <Button loading={harness.scanning} onclick={() => void harness.rescan()}>
        {#snippet leading()}<RotateCw size={16} aria-hidden="true" />{/snippet}
        {t('onboarding.harnesses.rescan')}
      </Button>
    </div>

    <!-- Arrow keys are a convenience on top of the radio buttons, which are individually focusable. -->
    <div role="radiogroup" aria-label={t('onboarding.harnesses.group')} tabindex="-1" class={cx(CARD_GRID, 'outline-none')} onkeydown={onKeydown}>
      {#each harness.summaries as summary (summary.info.id)}
        <HarnessCard {summary} selected={onboarding.driver === summary.info.id} onselect={() => onboarding.choose(summary.info.id)} />
      {/each}
    </div>

  {/if}
</div>
