<script lang="ts">
  import { untrack } from 'svelte';
  import { slide } from 'svelte/transition';

  import Switch from '$lib/components/Switch.svelte';
  import { scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { memoryCode } from '$lib/stores/memoryCode.svelte';

  import SettingRow from './SettingRow.svelte';

  const index = $derived(memoryCode.index);
  // The environment sets the switch for a server started with it: the row says so instead of letting a change through that would not stick.
  const description = $derived(t(memoryCode.locked ? 'settings.memory.code.descriptionEnv' : 'settings.memory.code.description'));

  // The switch is read again with every look at the settings: the row only shows once the server answered.
  $effect(() => {
    untrack(() => void memoryCode.load());
  });
</script>

{#if index}
  <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
    <SettingRow title={t('settings.memory.code.title')} {description}>
      {#snippet control()}
        <Switch label={t('settings.memory.code.title')} disabled={memoryCode.pending || memoryCode.locked} bind:checked={() => index?.enabled ?? false, (on) => void memoryCode.setEnabled(on)} />
      {/snippet}
    </SettingRow>

    {#if memoryCode.failure}
      <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
        <p class="pb-4 text-sm text-danger" role="alert">{memoryCode.failure}</p>
      </div>
    {/if}
  </div>
{/if}
