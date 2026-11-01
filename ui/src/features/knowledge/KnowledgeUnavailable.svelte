<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import Database from '@lucide/svelte/icons/database';
  import Download from '@lucide/svelte/icons/download';
  import Hourglass from '@lucide/svelte/icons/hourglass';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import WifiOff from '@lucide/svelte/icons/wifi-off';

  import Button from '$lib/components/Button.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import type { IconComponent } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeAvailability } from '$lib/ipc/bindings/KnowledgeAvailability';
  import { nav } from '$lib/stores/nav.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';

  import { problemText } from '../settings/knowledge-remote';
  import KnowledgeInstall from './KnowledgeInstall.svelte';

  interface Props {
    /** Why there is nothing to list: anything but `available`. */
    availability: Exclude<KnowledgeAvailability, { type: 'available' }>;
    onretry: () => void;
    /** Reads what the server says again, once Harness Knowledge was installed or updated. */
    onrefresh: () => Promise<void>;
  }

  let { availability, onretry, onrefresh }: Props = $props();

  const page = $derived.by((): { icon: IconComponent; title: string; body: string } => {
    switch (availability.type) {
      case 'notInstalled':
        return { icon: Download, title: t('knowledge.settings.install.missing.title'), body: t('knowledge.settings.install.missing.description') };
      case 'outdated':
        return { icon: RefreshCw, title: t('knowledge.settings.install.outdated.title'), body: t('knowledge.settings.install.outdated.description') };
      case 'missing':
        return { icon: Database, title: t('knowledge.unavailable.missing.title'), body: problemText(availability.problem, availability.message) };
      case 'unreachable':
        return { icon: WifiOff, title: t('knowledge.unreachable.title'), body: t('knowledge.unreachable.body') };
      case 'upgrading':
        return { icon: Hourglass, title: t('knowledge.upgrading.title'), body: t('knowledge.upgrading.body') };
      case 'failed':
        return { icon: CircleAlert, title: t('knowledge.unavailable.failed.title'), body: t('knowledge.unavailable.failed.body', { message: availability.message }) };
    }
  });

  function openSettings(): void {
    viewState.settingsTarget = 'knowledge';
    nav.go('settings');
  }
</script>

<!-- Not tied to Documents or Review: a missing or old Harness Knowledge is fixed right here, with the same installer the settings run. A database that lacks something is fixed in Settings. -->
<div class="grid size-full place-items-center">
  <EmptyState icon={page.icon} title={page.title} body={page.body}>
    {#snippet action()}
      {#if availability.type === 'upgrading'}
        <!-- A passing state: the list is read again on its own, so there is nothing to press. -->
      {:else if availability.type === 'failed' || availability.type === 'unreachable'}
        <Button variant="primary" onclick={onretry}>{t('memory.tryAgain')}</Button>
      {:else if availability.type === 'notInstalled' || availability.type === 'outdated'}
        <KnowledgeInstall update={availability.type === 'outdated'} onfinished={onrefresh} />
      {:else}
        <Button variant="primary" onclick={openSettings}>{t('knowledge.unavailable.openSettings')}</Button>
      {/if}
    {/snippet}
  </EmptyState>
</div>
