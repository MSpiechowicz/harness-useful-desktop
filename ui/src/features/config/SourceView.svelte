<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import { tick } from 'svelte';

  import { matchIndices, normalizeNeedle } from '$lib/components/config-tree';
  import ConfigValueView from '$lib/components/ConfigValueView.svelte';
  import VirtualList from '$lib/components/VirtualList.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { ConfigField } from '$lib/ipc/bindings/ConfigField';
  import type { ConfigFormat } from '$lib/ipc/bindings/ConfigFormat';
  import { cx } from '$lib/utils/cx';
  import { highlightSegments } from '$lib/utils/fuzzy';

  import { foldIds, lineMatches, toLines, visibleLines, type SourceLine, type VisibleLine } from './source';

  interface Props {
    fields: ConfigField[];
    /** Case-insensitive search over keys and plain values (never secrets). */
    query: string;
    /** Accessible name of the listing. */
    label: string;
    format: ConfigFormat;
    /** Out: how many blocks are open right now, so a parent can offer "Collapse all" or "Expand all". */
    openCount?: number;
  }

  let { fields, query, label, format, openCount = $bindable(0) }: Props = $props();

  /** Beyond this many lines on screen the list renders only the rows in view. */
  const VIRTUAL_LINES = 400;
  const LINE_PX = 22;

  let closed = $state.raw<ReadonlySet<string>>(new Set());
  /** The match `step` stopped at, as a line index; null until stepped. */
  let currentLine = $state<number | null>(null);
  let list: ReturnType<typeof VirtualList<VisibleLine>> | undefined = $state();
  let plain: HTMLElement | undefined = $state();

  const needle = $derived(normalizeNeedle(query));
  const lines = $derived(toLines(fields, format));
  const rows = $derived(visibleLines(lines, closed, needle));
  const hits = $derived(rows.flatMap((row, position) => (lineMatches(row.line, needle) ? [position] : [])));
  const virtual = $derived(rows.length > VIRTUAL_LINES);

  $effect(() => {
    openCount = rows.filter((row) => row.line.foldStart !== undefined && !row.folded).length;
  });

  // A new search starts over from its first match.
  $effect(() => {
    void needle;
    currentLine = null;
  });

  /** Opens or closes every block. */
  export function setAll(open: boolean): void {
    closed = open ? new Set() : new Set(foldIds(lines));
  }

  /** Moves to the next or previous match, wrapping around, and scrolls it into view. */
  export function step(direction: 1 | -1): void {
    if (hits.length === 0) {
      return;
    }

    const at = hits.findIndex((position) => rows[position].index === currentLine);
    const next = at === -1 ? (direction === 1 ? 0 : hits.length - 1) : (at + direction + hits.length) % hits.length;

    void goTo(hits[next]);
  }

  async function goTo(position: number): Promise<void> {
    currentLine = rows[position].index;
    await tick();

    if (virtual) {
      list?.scrollToIndex(Math.max(0, position - 5));
    } else {
      plain?.querySelector(`[data-index="${currentLine}"]`)?.scrollIntoView?.({ block: 'center' });
    }
  }

  function toggle(id: string): void {
    const next = new Set(closed);

    if (!next.delete(id)) {
      next.add(id);
    }

    closed = next;
  }

  /** A click on the minimap jumps to the match nearest to where it landed. */
  function jump(event: MouseEvent): void {
    const bar = event.currentTarget as HTMLElement;
    const box = bar.getBoundingClientRect();
    if (hits.length === 0 || box.height === 0) {
      return;
    }

    const target = ((event.clientY - box.top) / box.height) * rows.length;
    const nearest = hits.reduce((best, position) => (Math.abs(position - target) < Math.abs(best - target) ? position : best));

    void goTo(nearest);
  }

  function summary(line: SourceLine): string {
    return tn(line.valueKind === 'map' ? 'components.config.key' : 'components.config.item', line.foldCount ?? 0);
  }
</script>

{#snippet marked(text: string, on: boolean, current: boolean)}
  {#if on}
    {#each highlightSegments(text, matchIndices(text, needle)) as segment, index (index)}{#if segment.match}<mark class={cx('rounded-[2px] text-fg', current ? 'bg-warning/70' : 'bg-warning/35')}>{segment.text}</mark>{:else}{segment.text}{/if}{/each}
  {:else}
    {text}
  {/if}
{/snippet}

{#snippet lineRow(row: VisibleLine)}
  {@const line = row.line}
  {@const hit = lineMatches(line, needle)}
  {@const current = row.index === currentLine}
  {@const key = line.shownKey ?? ''}
  <div
    data-index={row.index}
    class={cx(
      'grid h-[22px] grid-cols-[40px_18px_minmax(0,1fr)] pr-5',
      hit ? 'bg-warning-wash' : 'hover:bg-hover',
      current && 'shadow-[inset_2px_0_0_var(--warning)]',
    )}
  >
    <span class="pr-[10px] text-right text-fg-3 tabular opacity-60 select-none" aria-hidden="true">{line.no}</span>

    <span class="grid place-items-center">
      {#if line.foldStart !== undefined}
        {@const id = line.foldStart}
        <button
          type="button"
          aria-expanded={!row.folded}
          aria-label={row.folded ? t('components.configView.source.unfold', { no: line.no }) : t('components.configView.source.fold', { no: line.no })}
          class="grid size-[18px] place-items-center rounded-sm text-fg-3 transition-colors duration-[var(--dur-fast)] hover:text-fg focus-visible:outline-2 focus-visible:outline-offset-[-2px]"
          onclick={() => toggle(id)}
        >
          <ChevronRight size={14} aria-hidden="true" class={cx('transition-transform duration-[var(--dur-fast)]', !row.folded && 'rotate-90')} />
        </button>
      {/if}
    </span>

    <!-- A flex row, so the template's own whitespace never becomes a space between the pieces of a line. -->
    <span class="flex min-w-0 items-center overflow-hidden whitespace-pre text-fg-2" style:padding-left="{line.depth * 2}ch">
      {#if line.bullet}<span class="text-fg-3">{'- '}</span>{/if}
      {#if line.key !== undefined}
        <span class="text-accent-text">{@render marked(key, hit, current)}</span>
        <span class="text-fg-3">{line.text !== '' || line.secret !== undefined ? ': ' : ':'}</span>
      {/if}
      {#if line.secret !== undefined}
        <ConfigValueView value={{ kind: 'secret', value: line.secret }} />
      {:else if line.valueKind === 'text'}
        <span class="text-success">{@render marked(line.text, hit, current)}</span>
      {:else if line.valueKind === 'number'}
        <span class="tabular text-warning">{@render marked(line.text, hit, current)}</span>
      {:else if line.valueKind === 'bool'}
        <span class="text-accent-text">{@render marked(line.text, hit, current)}</span>
      {:else if line.valueKind === 'null'}
        <span class="text-fg-3 italic">{@render marked(line.text, hit, current)}</span>
      {:else if line.text !== ''}
        <span>{line.text}</span>
      {/if}
      {#if line.comma}<span class="text-fg-3">,</span>{/if}
      {#if row.folded && line.foldStart !== undefined}
        {@const id = line.foldStart}
        <button
          type="button"
          class="ml-2 shrink-0 rounded-full bg-surface-3 px-2 font-sans text-[11px] leading-[18px] text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-surface-4 hover:text-fg focus-visible:outline-2"
          onclick={() => toggle(id)}
        >
          {summary(line)}
        </button>
      {/if}
    </span>
  </div>
{/snippet}

{#if fields.length === 0}
  <p class="px-3 py-6 text-center text-sm text-fg-3" role="status">{t('components.config.none')}</p>
{:else}
  <div class="relative flex h-full min-h-0 flex-col py-2 font-mono text-[12.5px] leading-[22px]">
    {#if virtual}
      <VirtualList bind:this={list} items={rows} estimate={LINE_PX} {label} key={(row) => row.index} overscan={12} class="min-h-0 flex-1">
        {#snippet row(item)}
          {@render lineRow(item)}
        {/snippet}
      </VirtualList>
    {:else}
      <div bind:this={plain} role="list" aria-label={label} class="min-h-0 flex-1 overflow-y-auto">
        {#each rows as row (row.index)}
          <div role="listitem">{@render lineRow(row)}</div>
        {/each}
      </div>
    {/if}

    {#if hits.length > 0}
      <!-- Pointer shortcut only: the keyboard steps through matches with the search field's Enter. -->
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div class="absolute top-2 right-[6px] bottom-2 w-[6px] cursor-pointer rounded-[3px] bg-surface-2" aria-hidden="true" onclick={jump}>
        {#each hits as position (position)}
          <i class="absolute inset-x-0 h-[3px] rounded-[2px] bg-warning opacity-80" style:top="{(position / rows.length) * 100}%"></i>
        {/each}
      </div>
    {/if}
  </div>
{/if}
