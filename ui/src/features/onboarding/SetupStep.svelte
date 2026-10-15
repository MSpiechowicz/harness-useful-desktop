<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import { onMount } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  import { WIDTH } from './layout';
  import SetupConfirm from './SetupConfirm.svelte';
  import SetupDashboardCard from './SetupDashboardCard.svelte';
  import SetupMatrix from './SetupMatrix.svelte';
  import { setup } from './setup-state.svelte';
  import StepHeader from './StepHeader.svelte';

  onMount(() => {
    void setup.open();
  });

  const data = $derived(setup.data);
  const selectedCount = $derived(setup.selectedKeys.length);
</script>

<div class={cx('mx-auto flex w-full flex-col gap-4 [@media(max-height:720px)]:gap-3', WIDTH.wide)}>
  <!-- The action sits beside the title, not under the cards: this step can be taller than the frame and scrolls, and the way to run it must stay in sight. -->
  <div class="mb-1 flex items-end justify-between gap-6 [@media(max-height:720px)]:mb-0">
    <StepHeader eyebrow={t('onboarding.setup.eyebrow')} title={t('onboarding.setup.title')} body={t('onboarding.setup.body')} />

    <!-- With nothing left to install the action has no use, so it is not there. -->
    {#if data && (selectedCount > 0 || setup.running)}
      <div class="flex shrink-0 flex-col items-end gap-2 pb-1">
        <p class="text-sm text-fg-3" role="status">
          {#if setup.running}
            {t('onboarding.setup.running')}
          {:else}
            {tn('onboarding.setup.selected', selectedCount)}
          {/if}
        </p>
        <Button variant="primary" loading={setup.running} onclick={() => setup.ask()}>{t('onboarding.setup.run')}</Button>
      </div>
    {/if}
  </div>

  {#if setup.load === 'error'}
    <div class="flex items-center gap-4 rounded-lg border border-line bg-danger-wash p-4" role="alert">
      <CircleAlert class="shrink-0 text-danger" size={18} aria-hidden="true" />
      <p class="min-w-0 flex-1 text-sm text-fg">{t('onboarding.setup.error', { message: setup.error })}</p>
      <Button onclick={() => void setup.open()}>{t('onboarding.setup.reload')}</Button>
    </div>
  {:else if !data}
    <div role="status" aria-label={t('onboarding.setup.looking')}>
      <Skeleton height="18rem" class="rounded-lg" />
    </div>
  {:else}
    <!-- The dashboard has the most to say (command, side effects, installer output), so it comes first with the full width; the matrix of harnesses follows it. -->
    <SetupDashboardCard dashboard={data.dashboard} />

    <!-- One card for every harness, however many there are: the rows grow, the layout does not. -->
    <SetupMatrix harnesses={data.harnesses} />
  {/if}
</div>

<SetupConfirm />
