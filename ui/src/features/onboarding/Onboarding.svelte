<script lang="ts">
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';
  import X from '@lucide/svelte/icons/x';
  import { fade } from 'svelte/transition';

  import { focusTrap } from '$lib/a11y/focus-trap';
  import Button from '$lib/components/Button.svelte';
  import IconButton from '$lib/components/IconButton.svelte';
  import { scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { motion } from '$lib/stores/motion.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';

  import { intro, INTRO_LEAD_MS } from '../intro/intro-state.svelte';
  import ApprovalStep from './ApprovalStep.svelte';
  import DoneStep from './DoneStep.svelte';
  import FamilyStep from './FamilyStep.svelte';
  import HarnessesStep from './HarnessesStep.svelte';
  import { STEP_WIDTH, WIDTH } from './layout';
  import { onboarding } from './onboarding-state.svelte';
  import ProjectStep from './ProjectStep.svelte';
  import { setup } from './setup-state.svelte';
  import SetupStep from './SetupStep.svelte';
  import { stepIn, stepOut } from './step-transition';
  import StepProgress from './StepProgress.svelte';
  import StepScroller from './StepScroller.svelte';
  import WelcomeStep from './WelcomeStep.svelte';

  let finishing = $state(false);

  /** Skipping the install is its own, quieter action, offered while there is something to install. */
  /** The setup step cannot tell what is missing until the machine has been read, so its action waits for that. */
  const readingSetup = $derived(onboarding.step === 'setup' && (setup.load === 'idle' || setup.load === 'loading'));

  const canSkipSetup = $derived(onboarding.step === 'setup' && setup.forward === 'install');

  /** Opening straight out of the intro, the setup waits for the lockup to leave; opened later, it appears at once. */
  function enterFade(): { delay: number; duration: number } {
    const afterLockup = intro.phase === 'handoff' && motion.level === 'full';

    return { delay: afterLockup ? INTRO_LEAD_MS : 0, duration: scaled(300) };
  }

  /** The setup step's primary label: it installs what is missing before going on. */
  const SETUP_FORWARD = {
    install: () => t('onboarding.action.installAndContinue'),
    anyway: () => t('onboarding.action.continueAnyway'),
    continue: () => t('onboarding.action.continue'),
  };

  /** Label of the primary action per step. */
  const primaryLabel = $derived.by(() => {
    switch (onboarding.step) {
      case 'welcome':
        return t('onboarding.action.getStarted');
      case 'harnesses':
        return harness.installed.length === 0 ? t('onboarding.action.continueWithout') : t('onboarding.action.continue');
      case 'project':
        return project.active ? t('onboarding.action.continue') : t('onboarding.action.skip');
      case 'approval':
        return t('onboarding.action.gotIt');
      case 'family':
        return t('onboarding.action.continue');
      case 'setup':
        return SETUP_FORWARD[setup.forward]();
      case 'done':
        return t('onboarding.action.open');
    }
  });

  async function advance(): Promise<void> {
    // Installing is what going on means here; the confirmation comes first and the step moves on by itself once the run is over.
    if (onboarding.step === 'setup' && setup.forward === 'install') {
      setup.ask();

      return;
    }

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

    {#if onboarding.rerun}
      <div class="absolute top-4 right-4 z-10" in:fade|global={enterFade()}>
        <IconButton icon={X} label={t('onboarding.close')} side="bottom" disabled={onboarding.busy} onclick={() => onboarding.dismiss()} />
      </div>
    {/if}

    <!--
      One fixed frame for every step, sized for the tallest one and centred in the window, so nothing jumps between
      steps: the progress bar is pinned to the top, the actions to the bottom; the hero sits centred between
      them on the welcome step; every later step starts at the same place (the title of step 2) and flows down.
      Only that middle area scrolls on short windows.
    -->
    <div class="relative flex h-full items-center" in:fade|global={enterFade()}>
      <div class="mx-auto flex h-full max-h-[60rem] w-full max-w-[60rem] flex-col gap-7 px-6 py-10 [@media(max-height:720px)]:gap-5 [@media(max-height:720px)]:py-6">
        <StepProgress />

        <StepScroller resetKey={onboarding.step}>
          {#key onboarding.step}
            <div
              class={cx(
                'step-build mx-auto w-full min-w-0',
                onboarding.step === 'welcome' ? '[align-self:safe_center]' : 'self-start',
                WIDTH[STEP_WIDTH[onboarding.step]],
              )}
              style:--dir={onboarding.direction}
              in:stepIn
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
              {:else if onboarding.step === 'family'}
                <FamilyStep />
              {:else if onboarding.step === 'setup'}
                <SetupStep />
              {:else}
                <DoneStep />
              {/if}
            </div>
          {/key}
        </StepScroller>

        <footer class={cx('flex w-full shrink-0 items-center', onboarding.index === 0 ? 'justify-center' : 'justify-between')}>
          {#if onboarding.index > 0}
            <!-- Back and the primary action are the wizard's navigation buttons: same size and width, Back the quieter one. -->
            <Button variant="secondary" size="lg" class="min-w-36" disabled={onboarding.busy} onclick={() => onboarding.back()}>
              {#snippet leading()}<ArrowLeft size={18} aria-hidden="true" />{/snippet}
              {t('onboarding.back')}
            </Button>
          {/if}

          <div class="flex items-center gap-3">
            {#if canSkipSetup}
              <Button variant="ghost" size="lg" class="min-w-36 hover:bg-surface-3" disabled={onboarding.busy} onclick={() => onboarding.next()}>{t('onboarding.action.skip')}</Button>
            {/if}

            <Button variant="primary" size="lg" class="min-w-36" loading={finishing || (onboarding.step === 'setup' && setup.running)} disabled={onboarding.busy || readingSetup} onclick={() => void advance()}>
              {primaryLabel}
              {#snippet trailing()}<ArrowRight size={18} aria-hidden="true" />{/snippet}
            </Button>
          </div>
        </footer>
      </div>
    </div>
  </div>
{/if}

<style>
  /*
   * The incoming step builds in two clean beats: the heading block, then each content block a moment later, gliding a
   * short way in from the side the user is heading towards (--dir). Whole blocks only, never their insides, so nothing
   * inside a card appears late. The motion tokens zero the travel and the stagger for reduced motion, the duration for off.
   */
  .step-build > :global(*) > :global(*),
  .step-build > :global(*) > :global([data-build-group]) > :global(*) {
    animation: step-piece var(--dur-enter) var(--ease-out-expo) both;
    animation-delay: calc((140ms + var(--n, 0) * 70ms) * var(--stagger));
  }

  .step-build > :global(*) > :global(:nth-child(2)) { --n: 1; }
  .step-build > :global(*) > :global(:nth-child(3)) { --n: 2; }
  .step-build > :global(*) > :global(:nth-child(n + 4)) { --n: 3; }

  /* A step may group blocks that share a column (`data-build-group`): the group stays still and its blocks build in turn, each with its own `--n`. */
  .step-build > :global(*) > :global([data-build-group]) { animation: none; }

  @keyframes step-piece {
    from {
      opacity: 0;
      transform: translateX(calc(var(--dir, 1) * var(--shift) * 3));
    }
  }
</style>
