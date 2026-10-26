<script lang="ts">
  import Languages from '@lucide/svelte/icons/languages';

  import { t } from '$lib/i18n/i18n.svelte';

  import ToggleFilter from './ToggleFilter.svelte';

  interface Props {
    /** Whether the list shows only the notes the server flagged as not written in English. */
    on: boolean;
    onchange: (on: boolean) => void;
  }

  let { on, onchange }: Props = $props();
</script>

<ToggleFilter icon={Languages} label={t('memory.lang.filter')} {on} {onchange} />
