<script lang="ts">
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';
  import X from '@lucide/svelte/icons/x';
  import { fade } from 'svelte/transition';

  import { focusTrap } from '$lib/a11y/focus-trap';
  import AuroraCanvas from '$lib/components/AuroraCanvas.svelte';
  import Button from '$lib/components/Button.svelte';
  import IconButton from '$lib/components/IconButton.svelte';
  import { createSpring, moveSpring, scaled } from '$lib/design/motion';
  import { harness } from '$lib/stores/harness.svelte';
  import { motion } from '$lib/stores/motion.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';

  import { intro, INTRO_LEAD_MS } from '../intro/intro.svelte';
  import ApprovalStep from './ApprovalStep.svelte';
  import DoneStep from './DoneStep.svelte';
  import HarnessesStep from './HarnessesStep.svelte';
  import { AURORA_INTENSITY, STEP_WIDTH, WIDTH } from './layout';
  import { onboarding } from './onboarding.svelte';
  import ProjectStep from './ProjectStep.svelte';
  import { stepIn, stepOut } from './step-transition';
  import StepProgress from './StepProgress.svelte';
  import WelcomeStep from './WelcomeStep.svelte';

  let finishing = $state(false);

  /** Opening straight out of the intro, the setup waits for the lockup to leave; opened later, it appears at once. */
  function enterFade(): { delay: number; duration: number } {
    const afterLockup = intro.phase === 'handoff' && motion.level === 'full';

    return { delay: afterLockup ? INTRO_LEAD_MS : 0, duration: scaled(300) };
  }

  // The backdrop glows brighter on the bookend steps and settles while the user is choosing.
  const aurora = createSpring(AURORA_INTENSITY.welcome);

  $effect(() => {
    moveSpring(aurora, AURORA_INTENSITY[onboarding.step]);
  });

  /** Label of the primary action per step. */
  const primaryLabel = $derived.by(() => {
    switch (onboarding.step) {
      case 'welcome':
        return 'Get started';
      case 'harnesses':
        return harness.installed.length === 0 ? 'Continue without a harness' : 'Continue';
      case 'project':
        return project.active ? 'Continue' : 'Skip for now';
      case 'approval':
        return 'Got it';
      case 'done':
        return 'Open Harness Desktop';
    }
  });

  async function advance(): Promise<void> {
    if (onboarding.step !== 'done') {
      onboarding.next();

      return;
    }

    finishing = true;
    await onboarding.finish();
    finishing = false;
  }
</script>

{#if onboarding.visible}
  <div
    role="dialog"
    aria-modal="true"
    aria-labelledby="onboarding-title"
    use:focusTrap={{ onEscape: onboarding.rerun ? () => onboarding.dismiss() : undefined }}
    out:fade={{ duration: scaled(300) }}
    class="fixed inset-0 z-[70] overflow-hidden bg-canvas text-fg outline-none"
  >
    <!-- The canvas colour above is opaque from the first frame, so the shell never shows through; only the layers on top fade in. -->
    <!-- The aurora dims in the light theme through the same opacity token the shell backdrop uses. -->
    <div class="pointer-events-none absolute inset-0" style:opacity="var(--aurora-opacity)" aria-hidden="true" in:fade|global={enterFade()}>
      <AuroraCanvas class="size-full" intensity={aurora.current} animated={motion.level === 'full'} />
    </div>

    {#if onboarding.rerun}
      <div class="absolute top-4 right-4 z-10" in:fade|global={enterFade()}>
        <IconButton icon={X} label="Close setup" side="bottom" onclick={() => onboarding.dismiss()} />
      </div>
    {/if}

    <!--
      One fixed frame for every step, sized for the tallest one and centred in the window, so nothing jumps between
      steps: the progress bar is pinned to the top, the actions to the bottom, and each step sits centred between
      them (no dead gap above the buttons). Only that middle area scrolls on short windows.
    -->
    <div class="relative flex h-full items-center" in:fade|global={enterFade()}>
      <div class="mx-auto flex h-full max-h-[52rem] w-full max-w-[60rem] flex-col gap-7 px-6 py-10 [@media(max-height:720px)]:gap-5 [@media(max-height:720px)]:py-6">
        <StepProgress />

        <!-- Stacked grid cell: the outgoing and incoming steps overlap while they slide. -->
        <div class="step-area grid min-h-0 flex-1 overflow-x-hidden overflow-y-auto *:col-start-1 *:row-start-1">
          {#key onboarding.step}
            <div
              class={cx('mx-auto w-full min-w-0 [align-self:safe_center]', WIDTH[STEP_WIDTH[onboarding.step]])}
              in:stepIn={{ direction: onboarding.direction }}
              out:stepOut={{ direction: onboarding.direction }}
            >
              {#if onboarding.step === 'welcome'}
                <WelcomeStep />
              {:else if onboarding.step === 'harnesses'}
                <HarnessesStep />
              {:else if onboarding.step === 'project'}
                <ProjectStep />
              {:else if onboarding.step === 'approval'}
                <ApprovalStep />
              {:else}
                <DoneStep />
              {/if}
            </div>
          {/key}
        </div>

        <footer class={cx('flex w-full shrink-0 items-center', onboarding.index === 0 ? 'justify-center' : 'justify-between')}>
          {#if onboarding.index > 0}
            <!-- Back and Continue are the wizard's two navigation buttons: same size and width, Back the quieter one. -->
            <Button variant="secondary" size="lg" class="min-w-36" onclick={() => onboarding.back()}>
              {#snippet leading()}<ArrowLeft size={18} aria-hidden="true" />{/snippet}
              Back
            </Button>
          {/if}

          <Button variant="primary" size="lg" class="min-w-36" loading={finishing} onclick={() => void advance()}>
            {primaryLabel}
            {#snippet trailing()}<ArrowRight size={18} aria-hidden="true" />{/snippet}
          </Button>
        </footer>
      </div>
    </div>
  </div>
{/if}

<style>
  /* No scrollbar line beside the step: when a step is taller than the frame, its bottom edge fades instead. */
  .step-area {
    scrollbar-width: none;
    -webkit-mask-image: linear-gradient(to bottom, black calc(100% - 28px), transparent);
    mask-image: linear-gradient(to bottom, black calc(100% - 28px), transparent);
  }

  .step-area::-webkit-scrollbar {
    display: none;
  }
</style>
