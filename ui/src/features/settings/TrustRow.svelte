<script lang="ts">
  import { slide } from 'svelte/transition';

  import Switch from '$lib/components/Switch.svelte';
  import { scaled } from '$lib/design/motion';

  import SettingRow from './SettingRow.svelte';

  interface Props {
    title: string;
    description: string;
    checked: boolean;
    onchange: (checked: boolean) => void;
  }

  let { title, description, checked, onchange }: Props = $props();
</script>

<!-- One trust question of the team database: it slides in with the item it is about. -->
<div transition:slide|global={{ duration: scaled(200) }} class="overflow-hidden">
  <SettingRow {title} {description}>
    {#snippet control()}
      <Switch label={title} bind:checked={() => checked, onchange} />
    {/snippet}
  </SettingRow>
</div>
