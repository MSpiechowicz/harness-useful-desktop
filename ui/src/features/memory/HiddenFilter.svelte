<script lang="ts">
  import EyeOff from '@lucide/svelte/icons/eye-off';

  import { t } from '$lib/i18n/i18n.svelte';

  import ToggleFilter from './ToggleFilter.svelte';

  interface Props {
    /** How many team notes are hidden on this computer. */
    count: number;
    /** Whether the list shows those notes. */
    on: boolean;
    onchange: (on: boolean) => void;
  }

  let { count, on, onchange }: Props = $props();
</script>

<!-- A switch between the notes agents see and the team notes hidden from them here. -->
<ToggleFilter icon={EyeOff} label={t('memory.hidden.filter', { count })} {on} {onchange} />
