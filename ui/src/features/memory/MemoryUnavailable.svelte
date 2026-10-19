<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import Download from '@lucide/svelte/icons/download';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';

  import Button from '$lib/components/Button.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import type { IconComponent } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MemoryScriptsAvailability } from '$lib/ipc/bindings/MemoryScriptsAvailability';

  import MemoryInstall from './MemoryInstall.svelte';

  interface Props {
    /** Why there is nothing to list: anything but `available`. */
    availability: Exclude<MemoryScriptsAvailability, { kind: 'available' }>;
    onretry: () => void;
    /** Reads the scripts again without a loading state, once Harness Memory was installed or updated. */
    onrefresh: () => Promise<void>;
  }

  let { availability, onretry, onrefresh }: Props = $props();

  interface Page {
    icon: IconComponent;
    title: string;
    body: string;
  }

  // Not tied to Notes or Scripts: a missing or old Harness Memory is fixed right here, with the same installer the setup runs.
  const page = $derived.by((): Page => {
    switch (availability.kind) {
      case 'notInstalled':
        return { icon: Download, title: t('memory.unavailable.notInstalled.title'), body: t('memory.unavailable.notInstalled.body') };
      case 'outdated':
        return { icon: RefreshCw, title: t('memory.unavailable.outdated.title'), body: t('memory.unavailable.outdated.body') };
      case 'failed':
        return { icon: CircleAlert, title: t('memory.unavailable.failed.title'), body: t('memory.unavailable.failed.body', { message: availability.message }) };
    }
  });
</script>

<div class="grid size-full place-items-center">
  <EmptyState icon={page.icon} title={page.title} body={page.body}>
    {#snippet action()}
      {#if availability.kind === 'failed'}
        <Button variant="primary" onclick={onretry}>{t('memory.tryAgain')}</Button>
      {:else}
        <MemoryInstall update={availability.kind === 'outdated'} onfinished={onrefresh} />
      {/if}
    {/snippet}
  </EmptyState>
</div>
