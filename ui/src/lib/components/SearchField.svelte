<script lang="ts">
  import Search from '@lucide/svelte/icons/search';
  import X from '@lucide/svelte/icons/x';

  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  import IconButton from './IconButton.svelte';

  interface Props {
    value: string;
    label: string;
    placeholder?: string;
    class?: string;
  }

  let { value = $bindable(''), label, placeholder, class: className }: Props = $props();
</script>

<div
  class={cx(
    'flex h-control items-center gap-2 rounded-md border border-line bg-surface-2 pr-1 pl-3 text-fg-3 transition-colors duration-[var(--dur-fast)] focus-within:border-focus hover:border-line-strong',
    className,
  )}
>
  <Search size={16} aria-hidden="true" />
  <input
    bind:value
    type="search"
    aria-label={label}
    placeholder={placeholder ?? t('components.filter.placeholder')}
    autocomplete="off"
    spellcheck="false"
    class="h-full min-w-0 flex-1 bg-transparent text-base text-fg outline-none placeholder:text-fg-3 [&::-webkit-search-cancel-button]:hidden"
  />
  {#if value !== ''}
    <IconButton icon={X} label={t('components.filter.clear')} class="size-7!" onclick={() => (value = '')} />
  {/if}
</div>
