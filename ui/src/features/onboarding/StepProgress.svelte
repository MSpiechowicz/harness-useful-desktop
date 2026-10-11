<script lang="ts">
  import { cx } from '$lib/utils/cx';

  import { onboarding } from './onboarding-state.svelte';
</script>

<div class="flex items-center justify-between">
  <ol class="flex items-center gap-2" aria-label="Setup progress">
    {#each onboarding.steps as step, index (step)}
      <li
        aria-current={step === onboarding.step ? 'step' : undefined}
        class={cx(
          'h-1.5 rounded-full transition-[width,background-color] duration-[var(--dur-slow)] ease-[var(--ease-out-expo)]',
          step === onboarding.step ? 'w-8 bg-accent-fill' : 'w-4',
          index < onboarding.index ? 'bg-accent/60' : step !== onboarding.step && 'bg-line-strong',
        )}
      >
        <span class="sr-only">Step {index + 1} of {onboarding.steps.length}</span>
      </li>
    {/each}
  </ol>

  <span class="tabular text-sm text-fg-3" aria-hidden="true">{onboarding.index + 1} / {onboarding.steps.length}</span>
</div>
