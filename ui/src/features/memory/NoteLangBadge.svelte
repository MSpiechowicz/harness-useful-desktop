<script lang="ts">
  import Languages from '@lucide/svelte/icons/languages';

  import Badge from '$lib/components/Badge.svelte';
  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
</script>

<!-- Beside the kind badge of a note the server flagged as not written in English. Icon only, so it fits the kind column. -->
<span class="inline-flex" title={t('memory.lang.badge')}>
  <Badge tone="warning" size="lg" class="px-2">
    <Languages size={ICON.sm} aria-hidden="true" />
    <span class="sr-only">{t('memory.lang.badge')}</span>
  </Badge>
</span>
