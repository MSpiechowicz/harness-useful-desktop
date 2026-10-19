<script lang="ts">
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';

  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';

  interface Props {
    /** One more line under the chip: what it means here. */
    hint?: string;
  }

  let { hint }: Props = $props();
</script>

<!-- A routine that runs tools without asking acts with nobody there to say no: the editor and the review both say so, in the same words. -->
<div class="flex flex-col gap-1">
  <span class="inline-flex h-7 w-fit items-center gap-1.5 rounded-full bg-warning-wash px-3 text-sm font-medium text-warning">
    <TriangleAlert size={ICON.sm} aria-hidden="true" />{t('routines.unattended.chip')}
  </span>
  {#if hint}<p class="text-xs text-fg-2">{hint}</p>{/if}
</div>
