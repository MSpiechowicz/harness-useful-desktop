<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import type { Snippet } from 'svelte';

  import { tn } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';
  import { formatCount } from '$lib/utils/format';
  import { splitPath } from '$lib/utils/paths';
  import { rovingIndex } from '$lib/utils/roving';

  import type { ChangeItem, ChangeSection, ChangeTone } from './change-list';

  /**
   * A list of changed things (files, documents) under foldable headings: a status letter, the name, the line counts and the
   * pending comments on each row. It is given plain data and reports choices through callbacks, so git and a document review
   * feed it alike. One row at a time is selected, arrow keys move between the rows that are in view.
   */
  interface Props {
    sections: ChangeSection[];
    /** The `id` of the selected row. */
    selectedId: string | null;
    /** The group's accessible name. */
    label: string;
    /** A row was chosen, by click or by arrow key. */
    onselect: (id: string) => void;
    /** A row was chosen by click, e.g. to put a drawer away. */
    onpick?: (id: string) => void;
    /** Whether a heading is open. */
    isOpen: (sectionId: string) => boolean;
    ontoggle: (sectionId: string) => void;
    /** Headings cannot be folded, e.g. while a search is showing its matches. */
    locked?: boolean;
    testid?: string;
    /** A menu on the right of a row. `menu.open` keeps the row above its neighbours while the menu is showing. */
    menu?: Snippet<[item: ChangeItem, menu: { open: boolean; setOpen: (open: boolean) => void }]>;
  }

  let { sections, selectedId, label, onselect, onpick, isOpen, ontoggle, locked = false, testid, menu }: Props = $props();

  const TONE_CLASSES: Record<ChangeTone, string> = {
    success: 'bg-success-wash text-success',
    warning: 'bg-warning-wash text-warning',
    danger: 'bg-danger-wash text-danger',
    info: 'bg-info-wash text-info',
    neutral: 'bg-surface-3 text-fg-2',
  };

  /** The rows a person can reach, top to bottom, for arrow-key navigation. */
  const reachable = $derived(sections.filter((section) => isOpen(section.id)).flatMap((section) => section.items));

  let buttons: Record<string, HTMLElement | undefined> = $state({});

  /** The row whose menu is open: it sits above its neighbours and keeps its button visible. */
  let menuFor = $state<string | null>(null);

  function onKeydown(event: KeyboardEvent, item: ChangeItem): void {
    const next = rovingIndex(event.key, reachable.findIndex((candidate) => candidate.id === item.id), reachable.length, 'vertical');
    if (next === null) {
      return;
    }

    event.preventDefault();
    onselect(reachable[next].id);
    buttons[reachable[next].id]?.focus();
  }
</script>

<div role="group" aria-label={label} class="flex flex-col gap-[14px] panel-inset py-3" data-testid={testid}>
  {#each sections as section (section.id)}
    {@const open = isOpen(section.id)}
    <section class="flex flex-col gap-[6px]">
      <button
        type="button"
        aria-expanded={open}
        disabled={locked}
        class="group/heading flex h-[28px] w-full items-center gap-[10px] rounded-sm panel-heading text-left text-[11px] leading-[16px] font-medium tracking-wide text-fg-3 uppercase transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg-2 focus-visible:outline-2 focus-visible:outline-offset-[-2px] disabled:hover:bg-transparent disabled:hover:text-fg-3"
        onclick={() => ontoggle(section.id)}
      >
        <span class="grid size-[16px] shrink-0 place-items-center"><ChevronRight size={14} aria-hidden="true" class={cx('transition-transform duration-[var(--dur-fast)]', open && 'rotate-90')} /></span>
        <span class="flex-1 truncate">{section.title}</span>
        <span class="tabular">{formatCount(section.items.length)}</span>
      </button>

      {#if open}
        <ul role="listbox" aria-label={section.title} class="flex flex-col gap-[4px]">
          {#each section.items as item (item.id)}
            {@const selected = item.id === selectedId}
            {@const parts = splitPath(item.label)}
            <!-- The button that chooses the row and the menu are siblings, as in the Sessions list: a button cannot hold a button. -->
            <li role="presentation" class={cx('group/row relative', menuFor === item.id && 'z-10')}>
              <button
                bind:this={buttons[item.id]}
                type="button"
                role="option"
                aria-selected={selected}
                tabindex={selected ? 0 : -1}
                title={item.label}
                class={cx(
                  // As in the Extensions list: the focus ring lies on the row's own border, so the scrolling list cannot clip it.
                  'flex w-full items-center gap-2.5 rounded-md border min-h-[36px] panel-row py-1 text-left transition-colors duration-[var(--dur-fast)] focus-visible:outline-offset-[-1px]',
                  selected ? 'border-accent/30 bg-accent-wash' : 'border-transparent hover:bg-hover active:bg-press',
                )}
                onclick={() => {
                  onselect(item.id);
                  onpick?.(item.id);
                }}
                onkeydown={(event) => onKeydown(event, item)}
              >
                <span class={cx('grid size-5 shrink-0 place-items-center rounded-sm font-mono text-[11px] font-semibold', TONE_CLASSES[item.tone])} title={item.letterLabel}>
                  {item.letter}<span class="sr-only">{` ${item.letterLabel}`}</span>
                </span>

                <span class="flex min-w-0 flex-1 flex-col">
                  <span class="truncate text-sm text-fg"><span class="text-fg-3">{parts.dir}</span>{parts.name}</span>
                  {#if item.sublabel}
                    <span class="truncate text-xs text-fg-3">{item.sublabel}</span>
                  {/if}
                </span>

                {#if item.comments > 0}
                  <span class="tabular rounded-full bg-accent-wash px-1.5 text-xs leading-5 text-accent-text" title={tn('changes.files.pending', item.comments)}>{item.comments}<span class="sr-only">{` ${tn('changes.files.commentWord', item.comments)}`}</span></span>
                {/if}

                {#if item.hits}
                  <span class="tabular rounded-full bg-warning-wash px-1.5 text-xs leading-5 text-warning" title={tn('changes.search.hitsIn', item.hits)}>{item.hits}<span class="sr-only">{` ${tn('changes.search.hitsIn', item.hits)}`}</span></span>
                {/if}

                {#if item.additions != null && item.deletions != null}
                  <span class="tabular shrink-0 text-xs">
                    <span class="text-success">+{item.additions}</span>
                    <span class="text-danger">−{item.deletions}</span>
                  </span>
                {/if}

                {#if menu}
                  <!-- Room for the menu button, so the counts stay in view beside it. -->
                  <span class="w-[var(--control-h)] shrink-0" aria-hidden="true"></span>
                {/if}
              </button>

              {#if menu}
                <div
                  class={cx(
                    'absolute top-1/2 right-1 -translate-y-1/2 opacity-0 transition-opacity duration-[var(--dur-fast)] focus-within:opacity-100 group-hover/row:opacity-100',
                    (selected || menuFor === item.id) && 'opacity-100',
                  )}
                >
                  {@render menu(item, { open: menuFor === item.id, setOpen: (next) => (menuFor = next ? item.id : menuFor === item.id ? null : menuFor) })}
                </div>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/each}
</div>
