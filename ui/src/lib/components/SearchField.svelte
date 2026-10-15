<script lang="ts">
  import Search from '@lucide/svelte/icons/search';
  import X from '@lucide/svelte/icons/x';

  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';
  import { matchesShortcut } from '$lib/utils/shortcuts';

  import IconButton from './IconButton.svelte';
  import Kbd from './Kbd.svelte';

  interface Props {
    value: string;
    label: string;
    placeholder?: string;
    class?: string;
    onkeydown?: (event: KeyboardEvent) => void;
    /** Shortcut that focuses the field while it is on screen, shown at its end while it is empty. */
    shortcut?: string;
    /** A short result line shown at the end of the field while it holds text, e.g. "3 comments in 2 files". */
    status?: string;
    /** Marks the input as the element a surrounding popover focuses when it opens. */
    autofocus?: boolean;
  }

  let { value = $bindable(''), label, placeholder, class: className, onkeydown, shortcut, status, autofocus = false }: Props = $props();

  let input: HTMLInputElement | undefined = $state();

  // The page's own find would miss this view's lists, so the shortcut brings focus here instead.
  function onWindowKey(event: KeyboardEvent): void {
    if (shortcut && input && matchesShortcut(event, shortcut)) {
      event.preventDefault();
      input.focus();
      input.select();
    }
  }
</script>

<svelte:window onkeydown={onWindowKey} />

<div
  class={cx(
    'flex h-control items-center gap-2 rounded-md border border-line bg-surface-2 pr-1 pl-3 text-fg-3 transition-colors duration-[var(--dur-fast)] focus-within:border-focus hover:border-line-strong',
    className,
  )}
>
  <Search size={16} aria-hidden="true" />
  <input
    bind:this={input}
    bind:value
    {onkeydown}
    type="search"
    aria-label={label}
    data-autofocus={autofocus ? '' : undefined}
    placeholder={placeholder ?? t('components.filter.placeholder')}
    autocomplete="off"
    spellcheck="false"
    class="h-full min-w-0 flex-1 bg-transparent text-base text-fg outline-none placeholder:text-fg-3 [&::-webkit-search-cancel-button]:hidden"
  />
  {#if value === '' && shortcut}
    <span class="mr-2 flex shrink-0 items-center"><Kbd keys={shortcut} small /></span>
  {/if}
  {#if value !== '' && status}
    <span class="shrink-0 pr-1 text-xs whitespace-nowrap text-fg-3" role="status">{status}</span>
  {/if}
  {#if value !== ''}
    <IconButton icon={X} label={t('components.filter.clear')} class="size-7!" onclick={() => (value = '')} />
  {/if}
</div>
