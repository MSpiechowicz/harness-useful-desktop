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
    transition:fade={{ duration: scaled(300) }}
    class="fixed inset-0 z-[70] overflow-hidden bg-canvas text-fg outline-none"
  >
    <!-- The aurora dims in the light theme through the same opacity token the shell backdrop uses. -->
    <div class="pointer-events-none absolute inset-0" style:opacity="var(--aurora-opacity)" aria-hidden="true">
      <AuroraCanvas class="size-full" intensity={aurora.current} animated={motion.level === 'full'} />
    </div>

    {#if onboarding.rerun}
      <div class="absolute top-4 right-4 z-10">
        <IconButton icon={X} label="Close setup" side="bottom" onclick={() => onboarding.dismiss()} />
      </div>
    {/if}

    <!-- The whole block (progress, step, actions) is centred; on short windows the page scrolls instead of clipping. -->
    <div class="relative h-full overflow-x-hidden overflow-y-auto">
      <div class="mx-auto flex min-h-full w-full max-w-[52rem] flex-col justify-center gap-8 px-6 py-14 [@media(max-height:720px)]:gap-5 [@media(max-height:720px)]:py-8">
        <div class={cx('mx-auto w-full', WIDTH[STEP_WIDTH[onboarding.step]])}>
          <StepProgress />
        </div>

        <!-- Stacked grid cell: the outgoing and incoming steps overlap while they slide. -->
        <div class="grid *:col-start-1 *:row-start-1">
          {#key onboarding.step}
            <div class="min-w-0" in:stepIn={{ direction: onboarding.direction }} out:stepOut={{ direction: onboarding.direction }}>
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

        <footer class={cx('mx-auto flex w-full items-center', WIDTH[STEP_WIDTH[onboarding.step]], onboarding.index === 0 ? 'justify-center' : 'justify-between')}>
          {#if onboarding.index > 0}
            <Button variant="ghost" onclick={() => onboarding.back()}>
              {#snippet leading()}<ArrowLeft size={16} aria-hidden="true" />{/snippet}
              Back
            </Button>
          {/if}

          <Button variant="primary" size="lg" loading={finishing} onclick={() => void advance()}>
            {primaryLabel}
            {#snippet trailing()}<ArrowRight size={18} aria-hidden="true" />{/snippet}
          </Button>
        </footer>
      </div>
    </div>
  </div>
{/if}
