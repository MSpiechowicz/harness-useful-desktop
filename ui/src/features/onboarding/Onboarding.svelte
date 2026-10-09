<script lang="ts">
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';
  import X from '@lucide/svelte/icons/x';
  import { fade } from 'svelte/transition';

  import { focusTrap } from '$lib/a11y/focus-trap';
  import AuroraCanvas from '$lib/components/AuroraCanvas.svelte';
  import Button from '$lib/components/Button.svelte';
  import IconButton from '$lib/components/IconButton.svelte';
  import { rise, scaled } from '$lib/design/motion';
  import { harness } from '$lib/stores/harness.svelte';
  import { motion } from '$lib/stores/motion.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';

  import ApprovalStep from './ApprovalStep.svelte';
  import DoneStep from './DoneStep.svelte';
  import HarnessesStep from './HarnessesStep.svelte';
  import { onboarding } from './onboarding.svelte';
  import ProjectStep from './ProjectStep.svelte';
  import WelcomeStep from './WelcomeStep.svelte';

  let finishing = $state(false);

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
      <AuroraCanvas class="size-full" intensity={0.55} animated={motion.level === 'full'} />
    </div>

    <div class="relative mx-auto flex h-full max-w-4xl flex-col gap-6 px-10 py-8">
      <div class="flex items-center justify-between">
        <ol class="flex items-center gap-2" aria-label="Setup progress">
          {#each onboarding.steps as step, index (step)}
            <li
              aria-current={step === onboarding.step ? 'step' : undefined}
              class={cx(
                'h-1.5 rounded-full transition-[width,background-color] duration-[var(--dur-slow)] ease-[var(--ease-out-expo)]',
                step === onboarding.step ? 'w-8 bg-[image:var(--brand-gradient)]' : 'w-4',
                index < onboarding.index ? 'bg-accent/60' : step !== onboarding.step && 'bg-line-strong',
              )}
            >
              <span class="sr-only">Step {index + 1} of {onboarding.steps.length}</span>
            </li>
          {/each}
        </ol>

        {#if onboarding.rerun}
          <IconButton icon={X} label="Close setup" side="bottom" onclick={() => onboarding.dismiss()} />
        {/if}
      </div>

      <!-- Stacked grid cell: the outgoing and incoming steps overlap during the transition. -->
      <div class="-mx-4 grid min-h-0 flex-1 overflow-y-auto px-4 *:col-start-1 *:row-start-1">
        {#key onboarding.step}
          <div class="py-2" in:rise={{ y: 14 * onboarding.direction, delay: 120, duration: 420 }} out:fade={{ duration: scaled(120) }}>
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

      <footer class="flex items-center justify-between">
        <div>
          {#if onboarding.index > 0}
            <Button variant="ghost" onclick={() => onboarding.back()}>
              {#snippet leading()}<ArrowLeft size={16} aria-hidden="true" />{/snippet}
              Back
            </Button>
          {/if}
        </div>

        <Button variant="primary" size="lg" loading={finishing} onclick={() => void advance()}>
          {primaryLabel}
          {#snippet trailing()}<ArrowRight size={18} aria-hidden="true" />{/snippet}
        </Button>
      </footer>
    </div>
  </div>
{/if}
