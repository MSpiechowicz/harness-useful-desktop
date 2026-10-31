<script lang="ts">
  import Download from '@lucide/svelte/icons/download';
  import { expoOut } from 'svelte/easing';
  import { slide } from 'svelte/transition';

  import Button from '$lib/components/Button.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { DURATION, scaled } from '$lib/design/motion';
  import { motion } from '$lib/stores/motion.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { retryable, updates } from '$lib/stores/updates.svelte';

  import { viewUpdates } from './actions';
  import UpdateBar from './UpdateBar.svelte';
  import { failureMessage, installerClosesApp } from './copy';
  import { cx } from '$lib/utils/cx';

  /** The rail shows the whole card; collapsed, only the icon remains. */
  let { expanded = true }: { expanded?: boolean } = $props();

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
        // Like the dashboard's card: under the title, what installing does to the running app.
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

  const fraction = $derived(updates.fraction);
  // The dashboard's compact button (28px, 12px text), like the rest of the rail.
  const SMALL = 'h-[28px]! gap-1.5! rounded-sm! px-3! text-xs!';

  const canRetry = $derived(state.kind === 'failed' && retryable(state.reason));
</script>

<!-- Settings shows the same update in full, so the notice would only repeat it there. -->
<!-- The Harness dashboard's card, above the language picker: the arrow inside the button bobs while an update waits. -->
{#snippet arrow()}
  <span class={cx('inline-flex', motion.level === 'full' && 'update-arrow')}><Download size={13} aria-hidden="true" /></span>
{/snippet}

{#if updates.noticeVisible && nav.current !== 'settings'}
  {#if expanded}
    <section
      aria-label={t('updates.notice.label')}
      transition:slide={{ duration: scaled(DURATION.slow), easing: expoOut }}
      class="shrink-0 overflow-hidden rounded-[8px] border border-line bg-accent-wash px-3 py-2.5 text-xs"
    >
      <div class="font-medium text-fg" role="status">{title}</div>
      {#if body}<div class="mt-0.5 text-fg-2">{body}</div>{/if}

      {#if working}
        <div class="mt-1 flex items-center gap-1.5 text-fg-2"><Spinner size={13} />{t(state.kind === 'installing' ? 'updates.progress.installing' : 'updates.progress.downloading')}{#if state.kind === 'downloading' && fraction !== null}<span class="tabular">{Math.round(fraction * 100)}%</span>{/if}</div>
        <div class="mt-2"><UpdateBar /></div>
      {:else}
        <div class="mt-2 flex items-center gap-2">
          {#if state.kind === 'available'}
            <Button variant="primary" class="{SMALL} flex-1" onclick={() => void updates.install()}>{@render arrow()}{t('updates.notice.install')}</Button>
            <Button variant="ghost" class={SMALL} onclick={() => updates.dismiss()}>{t('updates.notice.later')}</Button>
          {:else if state.kind === 'readyToRelaunch'}
            <Button variant="primary" class={SMALL} onclick={() => void updates.requestRelaunch()}>{updates.closesApp ? t('updates.notice.finish') : t('updates.notice.restart')}</Button>
            <Button variant="ghost" class={SMALL} onclick={() => updates.dismiss()}>{t('updates.notice.later')}</Button>
          {:else}
            {#if canRetry}
              <Button variant="primary" class={SMALL} onclick={() => void updates.install()}>{t('updates.notice.retry')}</Button>
            {/if}
            <Button variant="ghost" class={SMALL} onclick={viewUpdates}>{t('updates.notice.view')}</Button>
            <Button variant="ghost" class={SMALL} onclick={() => updates.dismiss()}>{t('updates.notice.dismiss')}</Button>
          {/if}
        </div>
      {/if}
    </section>
  {:else}
    <button
      type="button"
      onclick={viewUpdates}
      title={title}
      aria-label="{title}. {t('updates.notice.view')}"
      class="mx-auto inline-flex size-control items-center justify-center rounded-md text-accent-text hover:bg-hover"
    >
      {#if working}<Spinner size={16} />{:else}{@render arrow()}{/if}
    </button>
  {/if}
{/if}
