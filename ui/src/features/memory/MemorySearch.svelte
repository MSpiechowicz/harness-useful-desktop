<script lang="ts">
  import type { Snippet } from 'svelte';

  import SearchField from '$lib/components/SearchField.svelte';

  import HeaderActions from './HeaderActions.svelte';

  interface Props {
    value: string;
    label: string;
    placeholder: string;
    /** More controls in the header, before the search field. */
    before?: Snippet;
  }

  let { value = $bindable(), label, placeholder, before }: Props = $props();

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape' && value !== '') {
      event.preventDefault();
      event.stopPropagation();
      value = '';
    }
  }
</script>

<HeaderActions>
  {@render before?.()}

  <div class="search-width">
    <SearchField bind:value {label} {placeholder} shortcut="mod+f" onkeydown={onKeydown} />
  </div>
</HeaderActions>
