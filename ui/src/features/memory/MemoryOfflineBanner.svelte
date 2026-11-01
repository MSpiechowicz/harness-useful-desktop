<script lang="ts">
  import OfflineBanner from '$lib/components/OfflineBanner.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { relativeTime } from '$lib/utils/format';

  interface Props {
    /** The page on screen: its `offline` and `cachedAt`. Null while nothing was read. */
    page: { offline: boolean; cachedAt: string | null } | null;
  }

  let { page }: Props = $props();
</script>

<!-- The list is the copy kept when the team database was last reachable, or only what is on this computer. -->
<OfflineBanner offline={page?.offline ?? false} message={page?.cachedAt ? t('memory.offline.cached', { time: relativeTime(page.cachedAt) }) : t('memory.offline.empty')} />
