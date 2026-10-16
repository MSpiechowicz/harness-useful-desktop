<script lang="ts">
  import { onMount } from 'svelte';

  import { scaled } from '$lib/design/motion';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  import { WIDTH } from './layout';
  import { onboarding } from './onboarding-state.svelte';
  import SetupConfirm from './SetupConfirm.svelte';
  import SetupInstallers from './SetupInstallers.svelte';
  import SetupMatrix from './SetupMatrix.svelte';
  import { setup } from './setup-state.svelte';
  import SetupStatus from './SetupStatus.svelte';
  import StepHeader from './StepHeader.svelte';

  // The family step before this one has read the machine already; only a flow entered here has to do it.
  onMount(() => {
    if (setup.load === 'idle') {
      void setup.open();
    }
  });

  const selectedCount = $derived(setup.selectedKeys.length);

  /** Once the confirmed run is over the step goes on by itself, unless something failed: then its rows stay for the user to read, retry or go on past. */
  async function runAndContinue(): Promise<void> {
    await setup.run();

    if (setup.hasFailed) {
      return;
    }

    // A moment on the finished rows, so the result is seen before the step changes.
    await new Promise((resolve) => setTimeout(resolve, scaled(300)));

    if (onboarding.step === 'setup') {
      onboarding.next();
    }
  }
</script>

<div class={cx('mx-auto flex w-full flex-col gap-4 [@media(max-height:720px)]:gap-3', WIDTH.wide)}>
  <div class="mb-1 flex items-end justify-between gap-6 [@media(max-height:720px)]:mb-0">
    <StepHeader eyebrow={t('onboarding.setup.eyebrow')} title={t('onboarding.setup.title')} body={t('onboarding.setup.body')} />

    <!-- This step can be taller than the frame and scrolls, so the way forward is the footer's primary action, which installs what is missing; the title only says how much that is. -->
    {#if setup.data && (selectedCount > 0 || setup.running)}
      <p class="shrink-0 pb-1 text-sm text-fg-3" role="status">
        {#if setup.running}
          {t('onboarding.setup.running')}
        {:else}
          {tn('onboarding.setup.selected', selectedCount)}
        {/if}
      </p>
    {/if}
  </div>

  <SetupStatus placeholder="18rem">
    {#snippet children(data)}
      <!-- The family's installers and skills run first, so they come first, with their output; the table of MCP servers per harness follows. One card for every harness, however many there are: the rows grow, the layout does not. -->
      <SetupInstallers {data} />
      <SetupMatrix harnesses={data.harnesses} />
    {/snippet}
  </SetupStatus>
</div>

<SetupConfirm bind:open={setup.confirming} plan={setup.plan} onconfirm={() => void runAndContinue()} />
