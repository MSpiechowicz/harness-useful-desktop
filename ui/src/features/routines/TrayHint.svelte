<script lang="ts">
  import Power from '@lucide/svelte/icons/power';

  import Button from '$lib/components/Button.svelte';
  import DocNoticeStrip from '$lib/components/docs/DocNoticeStrip.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  interface Props {
    /** The switch "Keep running in the tray when the window is closed" is off while some routine is on. */
    show: boolean;
    /** Turns that switch on. */
    onTurnOn: () => void | Promise<void>;
  }

  let { show, onTurnOn }: Props = $props();

  let working = $state(false);

  async function turnOn(): Promise<void> {
    working = true;

    try {
      await onTurnOn();
    } finally {
      working = false;
    }
  }
</script>

<!-- Routines only run while Harness Desktop does: with the window closed, nothing runs unless the app stays in the tray. -->
{#if show}
  <DocNoticeStrip icon={Power} title={t('routines.tray.title')} body={t('routines.tray.body')}>
    {#snippet action()}
      <Button loading={working} onclick={() => void turnOn()}>{t('routines.tray.turnOn')}</Button>
    {/snippet}
  </DocNoticeStrip>
{/if}
