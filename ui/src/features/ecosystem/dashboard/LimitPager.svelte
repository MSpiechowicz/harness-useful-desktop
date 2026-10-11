<script lang="ts">
  import { planSubject, type LimitGroup } from '$lib/dashboard/limits';
  import { cx } from '$lib/utils/cx';
  import { rovingIndex } from '$lib/utils/roving';

  interface Props {
    groups: LimitGroup[];
    /** The shown page, 0-based. */
    page: number;
    onpage: (page: number) => void;
    /** Prefix of the ids that tie each tab to its page. */
    idBase: string;
  }

  let { groups, page, onpage, idBase }: Props = $props();

  let tabs: (HTMLButtonElement | undefined)[] = $state([]);

  function onKeydown(event: KeyboardEvent, index: number): void {
    const next = rovingIndex(event.key, index, groups.length, 'horizontal');
    if (next === null) {
      return;
    }

    event.preventDefault();
    onpage(next);
    tabs[next]?.focus();
  }
</script>

<div role="tablist" aria-label="Provider" class="inline-flex h-8 items-center rounded-md border border-line bg-surface-2 p-0.5">
  {#each groups as group, index (group.key)}
    {@const selected = index === page}
    {@const name = planSubject(group.provider, group.plan)}
    <button
      bind:this={tabs[index]}
      type="button"
      role="tab"
      id="{idBase}-tab-{index}"
      aria-selected={selected}
      aria-controls="{idBase}-page-{index}"
      aria-label="Provider {index + 1} of {groups.length}: {name}"
      tabindex={selected ? 0 : -1}
      onclick={() => onpage(index)}
      onkeydown={(event) => onKeydown(event, index)}
      class={cx(
        'inline-flex h-full items-center gap-2 rounded-sm border px-3 text-sm font-medium transition-[background-color,color,border-color,box-shadow] duration-[var(--dur-fast)]',
        selected ? 'border-line-strong bg-surface-4 text-fg shadow-raised' : 'border-transparent text-fg-2 hover:text-fg',
      )}
    >
      {name}
    </button>
  {/each}
</div>
