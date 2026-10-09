<script lang="ts">
  import RotateCw from '@lucide/svelte/icons/rotate-cw';

  import Button from '$lib/components/Button.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { cx } from '$lib/utils/cx';
  import { rovingIndex } from '$lib/utils/roving';

  import HarnessCard from './HarnessCard.svelte';
  import { WIDTH } from './layout';
  import { onboarding } from './onboarding.svelte';
  import StepHeader from './StepHeader.svelte';

  const installed = $derived(harness.installed);

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
    eyebrow="Your harnesses"
    title="Which agent do you use every day?"
    body="We looked for OMP and PI on this machine. Pick your daily driver; you can switch any time from the top bar."
  />

  {#if harness.status === 'loading' || harness.status === 'idle'}
    <div class="grid grid-cols-2 gap-4" role="status" aria-label="Looking for harnesses">
      <Skeleton height="16rem" />
      <Skeleton height="16rem" />
    </div>
  {:else if harness.status === 'error'}
    <p class="rounded-md border border-line bg-danger-wash p-4 text-sm text-fg" role="alert">
      Could not scan for harnesses: {harness.error?.message}
    </p>
  {:else}
    <!-- What the scan found, with the way to scan again right next to it. -->
    <div class="-mb-1 flex items-center justify-between gap-3">
      <p class="text-sm text-fg-3">
        {#if installed.length === 0}
          No harness found yet. You can continue and install one later.
        {:else}
          Found {installed.length} of {harness.summaries.length} on this machine.
        {/if}
      </p>
      <Button variant="ghost" size="sm" loading={harness.scanning} onclick={() => void harness.rescan()}>
        {#snippet leading()}<RotateCw size={16} aria-hidden="true" />{/snippet}
        Re-scan
      </Button>
    </div>

    <!-- Arrow keys are a convenience on top of the radio buttons, which are individually focusable. -->
    <div role="radiogroup" aria-label="Daily driver" tabindex="-1" class="grid grid-cols-2 items-start gap-4 outline-none" onkeydown={onKeydown}>
      {#each harness.summaries as summary (summary.info.id)}
        <HarnessCard {summary} selected={onboarding.driver === summary.info.id} onselect={() => onboarding.choose(summary.info.id)} />
      {/each}
    </div>

  {/if}
</div>
