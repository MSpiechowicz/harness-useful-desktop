<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import Popover from '$lib/components/Popover.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  interface Props {
    /** The file updates the document it matches (else it is added as new). */
    update: boolean;
    /** The file, for the name of the menu. */
    name: string;
    /** The title of the document the file would replace, for the hint under "Update". */
    title: string;
    /** The import stores its documents as drafts (team approval is on, or the user asked): an update then becomes a pending update that waits for approval. */
    draft: boolean;
    onchange: (update: boolean) => void;
  }

  let { update, name, title, draft, onchange }: Props = $props();

  const choice = $derived(update ? t('knowledge.match.update') : t('knowledge.match.addNew'));

  const options = $derived([
    { value: true, label: t('knowledge.match.update'), hint: draft ? t('knowledge.drafts.match.update.hint') : t('knowledge.match.update.hint', { title }) },
    { value: false, label: t('knowledge.match.addNew'), hint: t('knowledge.match.addNew.hint') },
  ]);
</script>

<!-- The menu opens in the window's layer, so the table and the dialog around it never cut it off. -->
<Popover label={t('knowledge.match.choose', { name })} kind="menu" side="auto" align="end" matchTrigger={false} rootClass="w-full" class="w-72 min-w-0!">
  {#snippet trigger({ props, open })}
    <Button {...props} aria-label={`${t('knowledge.match.choose', { name })}: ${choice}`} title={update && draft ? t('knowledge.drafts.match.update.hint') : undefined} class="w-full justify-between">
      {choice}
      {#snippet trailing()}<ChevronDown size={ICON.sm} class={cx('transition-transform duration-[var(--dur-base)] ease-[var(--ease-out-expo)]', open && 'rotate-180')} aria-hidden="true" />{/snippet}
    </Button>
  {/snippet}

  {#snippet children({ close })}
    {#each options as option (option.label)}
      <button
        type="button"
        role="menuitemradio"
        aria-checked={option.value === update}
        data-menu-item
        class="flex w-full min-w-0 items-center gap-2.5 rounded-sm px-2.5 py-2 text-left transition-colors duration-[var(--dur-fast)] hover:bg-hover focus-visible:bg-hover"
        onclick={() => {
          close();
          onchange(option.value);
        }}
      >
        <span class="flex min-w-0 flex-1 flex-col">
          <span class={cx('truncate text-sm', option.value === update ? 'text-fg' : 'text-fg-2')}>{option.label}</span>
          <span class="truncate text-xs text-fg-3">{option.hint}</span>
        </span>
        {#if option.value === update}<Check size={ICON.sm} class="shrink-0 text-accent-text" aria-hidden="true" />{/if}
      </button>
    {/each}
  {/snippet}
</Popover>
