<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import Download from '@lucide/svelte/icons/download';
  import X from '@lucide/svelte/icons/x';
  import { expoOut } from 'svelte/easing';
  import { slide } from 'svelte/transition';

  import Button from '$lib/components/Button.svelte';
  import IconButton from '$lib/components/IconButton.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { DURATION, scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { retryable, updates } from '$lib/stores/updates.svelte';

  import { viewUpdates } from './actions';
  import { failureMessage, installerClosesApp } from './copy';
  import UpdateProgress from './UpdateProgress.svelte';

  const state = $derived(updates.state);
  const version = $derived(updates.info?.version ?? '');
  const working = $derived(state.kind === 'downloading' || state.kind === 'installing');

  const title = $derived.by(() => {
    switch (state.kind) {
      case 'available':
        return t('updates.notice.available', { version });

      case 'downloading':
      case 'installing':
        return t('updates.notice.updating', { version });

      case 'readyToRelaunch':
        return t('updates.notice.installed', { version });

      default:
        return t('updates.notice.failed');
    }
  });

  const body = $derived.by(() => {
    switch (state.kind) {
      case 'available':
        return updates.closesApp ? installerClosesApp() : t('updates.notice.availableBody');

      case 'downloading':
      case 'installing':
        return updates.closesApp ? installerClosesApp() : '';

      case 'readyToRelaunch':
        return updates.closesApp ? installerClosesApp() : t('updates.notice.restartBody');

      case 'failed':
        return failureMessage(state.reason, 'install');

      default:
        return '';
    }
  });

  const canRetry = $derived(state.kind === 'failed' && retryable(state.reason));
</script>

<!-- Settings shows the same update in full, so the notice would only repeat it there. -->
<!-- A band between the top bar and the view, in the flow of the page, so it pushes content down instead of covering it. -->
{#if updates.noticeVisible && nav.current !== 'settings'}
  <section
    aria-label={t('updates.notice.label')}
    transition:slide={{ duration: scaled(DURATION.slow), easing: expoOut }}
    class="shrink-0 overflow-hidden border-b border-line bg-surface-2"
  >
    <div class="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-2">
      <span class="shrink-0" aria-hidden="true">
        {#if working}
          <Spinner size={18} class="text-accent-text" />
        {:else if state.kind === 'readyToRelaunch'}
          <CircleCheck class="text-success" size={18} />
        {:else if state.kind === 'failed'}
          <CircleAlert class="text-warning" size={18} />
        {:else}
          <Download class="text-accent-text" size={18} />
        {/if}
      </span>

      <!-- Title and detail share a line while there is room, so the band costs one row instead of two. -->
      <div class="flex min-w-48 flex-1 flex-wrap items-baseline gap-x-3" role="status">
        <p class="text-base font-medium text-fg">{title}</p>
        {#if body}<p class="text-sm text-fg-2">{body}</p>{/if}
      </div>

      {#if working}
        <UpdateProgress inline />
      {/if}

      <div class="flex flex-wrap items-center gap-2">
        {#if state.kind === 'available'}
          <Button size="sm" variant="primary" onclick={() => void updates.install()}>{t('updates.notice.install')}</Button>
          <Button size="sm" onclick={viewUpdates}>{t('updates.notice.view')}</Button>
          <Button size="sm" variant="ghost" onclick={() => updates.dismiss()}>{t('updates.notice.later')}</Button>
        {:else if state.kind === 'readyToRelaunch'}
          <Button size="sm" variant="primary" onclick={() => void updates.requestRelaunch()}>{updates.closesApp ? t('updates.notice.finish') : t('updates.notice.restart')}</Button>
          <Button size="sm" variant="ghost" onclick={() => updates.dismiss()}>{t('updates.notice.later')}</Button>
        {:else if state.kind === 'failed'}
          {#if canRetry}
            <Button size="sm" onclick={() => void updates.install()}>{t('updates.notice.retry')}</Button>
          {/if}
          <Button size="sm" variant="ghost" onclick={viewUpdates}>{t('updates.notice.view')}</Button>
        {:else}
          <Button size="sm" variant="ghost" onclick={viewUpdates}>{t('updates.notice.view')}</Button>
        {/if}

        <IconButton icon={X} label={t('updates.notice.dismiss')} size="sm" onclick={() => updates.dismiss()} />
      </div>
    </div>
  </section>
{/if}
