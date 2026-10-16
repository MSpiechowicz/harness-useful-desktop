<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';

  import Button from '$lib/components/Button.svelte';
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import { ICON } from '$lib/components/icon';
  import MenuItem from '$lib/components/MenuItem.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { harness } from '$lib/stores/harness.svelte';

  const current = $derived(harness.active);
  // Only detected harnesses are offered, and with a single one there is nothing to switch to.
  const choices = $derived(harness.installed);
  const switchable = $derived(choices.length >= 2);
</script>

<!-- Picks the harness the Config and Extensions views show, the same choice as the palette's "Switch harness". -->
{#if switchable && current}
  <Popover label={t('harness.switcher.label')} kind="menu" align="end" class="w-60 min-w-0!">
    {#snippet trigger({ props })}
      <Button {...props} aria-label={`${t('harness.switcher.label')} (${current.info.shortName})`}>
        {#snippet leading()}<HarnessBadge info={current.info} size="xs" />{/snippet}
        {current.info.shortName}
        {#snippet trailing()}<ChevronDown size={ICON.sm} aria-hidden="true" />{/snippet}
      </Button>
    {/snippet}

    {#snippet children({ close })}
      <div role="none" class="flex flex-col">
        {#each choices as item (item.info.id)}
          {@const chosen = item.info.id === current.info.id}
          <MenuItem
            role="menuitemradio"
            aria-checked={chosen}
            class="gap-2.5"
            onclick={() => {
              close();
              void harness.select(item.info.id);
            }}
          >
            <HarnessBadge info={item.info} size="xs" />
            <span class="min-w-0 flex-1 truncate">{item.info.name}</span>
            <Check size={ICON.sm} aria-hidden="true" class={chosen ? 'shrink-0 text-accent-text' : 'invisible shrink-0'} />
          </MenuItem>
        {/each}
      </div>
    {/snippet}
  </Popover>
{/if}
