<script lang="ts" generics="Item">
  import type { Snippet } from 'svelte';

  import { cx } from '$lib/utils/cx';

  interface Props {
    id: string;
    label: string;
    items: Item[];
    active: number;
    /** Classes added to every row, for how its parts line up. */
    rowClass?: string;
    itemKey: (item: Item) => string;
    onaccept: (item: Item) => void;
    onhover: (index: number) => void;
    row: Snippet<[Item]>;
  }

  let { id, label, items, active, rowClass, itemKey, onaccept, onhover, row }: Props = $props();

  let list: HTMLUListElement | undefined = $state();

  // Keeps the active row inside the list when the arrows move it. The list's own scroll position moves, never an ancestor's.
  $effect(() => {
    const target = list?.children[active] as HTMLElement | undefined;
    if (!list || !target) {
      return;
    }

    if (target.offsetTop < list.scrollTop) {
      list.scrollTop = active === 0 ? 0 : target.offsetTop;
    } else if (target.offsetTop + target.offsetHeight > list.scrollTop + list.clientHeight) {
      list.scrollTop = active === items.length - 1 ? list.scrollHeight : target.offsetTop + target.offsetHeight - list.clientHeight;
    }
  });
</script>

<!-- Pointer picks only; the textarea keeps focus and drives the list with the keyboard (aria-activedescendant). -->
<ul bind:this={list} {id} role="listbox" aria-label={label} class="absolute inset-x-0 bottom-full z-30 mb-2 max-h-72 overflow-y-auto rounded-lg border border-line-strong bg-glass p-1.5 shadow-pop backdrop-blur-xl">
  {#each items as item, index (itemKey(item))}
    <!-- svelte-ignore a11y_click_events_have_key_events -- the textarea owns the keyboard -->
    <li
      id="{id}-{index}"
      role="option"
      aria-selected={index === active}
      class={cx('flex cursor-pointer gap-3 rounded-sm px-2.5 py-1.5 text-sm', rowClass, index === active ? 'bg-hover' : '')}
      onmousedown={(event) => event.preventDefault()}
      onclick={() => onaccept(item)}
      onpointerenter={() => onhover(index)}
    >
      {@render row(item)}
    </li>
  {/each}
</ul>
