<script lang="ts">
  import { ICON } from '$lib/components/icon';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import { t } from '$lib/i18n/i18n.svelte';
  import { relativeTime } from '$lib/utils/format';

  interface Props {
    /** The page on screen: its `offline` and `cachedAt`. Null while nothing was read. */
    page: { offline: boolean; cachedAt: string | null } | null;
  }

  let { page }: Props = $props();

  const look = NOTICE_LOOKS.warning;
</script>

<!-- While the team database cannot be reached the list is the copy kept when it last could be (or only what is on this computer). This says so, so it is not taken for what is kept. -->
{#if page?.offline}
  <div class="flex shrink-0 items-center gap-3 border-b border-line px-4 py-2 {look.box}" role="status">
    <span class="flex h-5 shrink-0 items-center"><look.icon class={look.color} size={ICON.sm} aria-hidden="true" /></span>
    <p class="min-w-0 flex-1 text-sm text-fg">{page.cachedAt ? t('memory.offline.cached', { time: relativeTime(page.cachedAt) }) : t('memory.offline.empty')}</p>
  </div>
{/if}
