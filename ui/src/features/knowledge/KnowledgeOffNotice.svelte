<script lang="ts">
  import { ICON } from '$lib/components/icon';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import Button from '$lib/components/Button.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';

  const look = NOTICE_LOOKS.info;

  function openSettings(): void {
    viewState.settingsTarget = 'knowledge';
    nav.go('settings');
  }
</script>

<!-- The documents are still here to look after, but no harness searches them while Project knowledge is off. -->
{#if prefs.value?.knowledgeEnabled === false}
  <div class="flex shrink-0 items-center gap-3 border-b border-line px-4 py-2 {look.box}" role="status">
    <span class="flex h-5 shrink-0 items-center"><look.icon class={look.color} size={ICON.sm} aria-hidden="true" /></span>
    <p class="min-w-0 flex-1 text-sm text-fg">{t('knowledge.off.notice')}</p>
    <Button variant="ghost" onclick={openSettings}>{t('knowledge.unavailable.openSettings')}</Button>
  </div>
{/if}
