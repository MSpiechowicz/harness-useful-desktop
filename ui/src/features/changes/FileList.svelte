<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';

  import { entrySections, SCOPE_LABELS, SECTIONS, statusLetter, statusName, statusTone, type Section, type StatusTone } from '$lib/git/entries';
  import { normalizeQuery } from '$lib/git/search';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { StatusEntry } from '$lib/ipc/bindings/StatusEntry';
  import { git } from '$lib/stores/git.svelte';
  import { cx } from '$lib/utils/cx';
  import { formatCount } from '$lib/utils/format';
  import { rovingIndex } from '$lib/utils/roving';

  import { folded } from './sections.svelte';
  import { splitPath } from './paths';

  interface Props {
    entries: StatusEntry[];
    /** Pending review comments per path. */
    commentCounts: ReadonlyMap<string, number>;
  }

  let { entries, commentCounts }: Props = $props();

  const TONE_CLASSES: Record<StatusTone, string> = {
    success: 'bg-success-wash text-success',
    warning: 'bg-warning-wash text-warning',
    danger: 'bg-danger-wash text-danger',
    info: 'bg-info-wash text-info',
    neutral: 'bg-surface-3 text-fg-2',
  };

  interface Row {
    entry: StatusEntry;
    section: Section;
  }

  /** The files under each heading; a file that is both staged and edited is listed under both. */
  const groups = $derived(
    SECTIONS.map((section) => ({ section, rows: entries.filter((entry) => entrySections(entry).includes(section)).map((entry): Row => ({ entry, section })) })).filter((group) => group.rows.length > 0),
  );

  /** While searching, every heading stays open so no match is hidden behind a fold. */
  const searching = $derived(normalizeQuery(git.search) !== null);
  const isOpen = (section: Section): boolean => searching || !folded.has(section);

  /** The rows a person can reach, top to bottom, for arrow-key navigation. */
  const reachable = $derived(groups.filter((group) => isOpen(group.section)).flatMap((group) => group.rows));

  let buttons: Record<string, HTMLElement | undefined> = $state({});

  const keyOf = (row: Row): string => `${row.section}:${row.entry.path}`;
  const isSelected = (row: Row): boolean => row.entry.path === git.selected && row.section === git.section;

  /** Added and removed lines of a file under a heading, once its diff has loaded. */
  function stats(path: string, section: Section): { additions: number; deletions: number } | null {
    const state = git.diffFor(path);
    if (state?.status !== 'ready') {
      return null;
    }

    const own = state.diffs.filter((diff) => diff.staged === (section === 'staged'));
    const shown = own.length > 0 ? own : state.diffs;
    if (shown.length === 0 || shown.every((diff) => diff.isBinary)) {
      return null;
    }

    return {
      additions: shown.reduce((sum, diff) => sum + diff.additions, 0),
      deletions: shown.reduce((sum, diff) => sum + diff.deletions, 0),
    };
  }

  function onKeydown(event: KeyboardEvent, row: Row): void {
    const next = rovingIndex(event.key, reachable.findIndex((candidate) => keyOf(candidate) === keyOf(row)), reachable.length, 'vertical');
    if (next === null) {
      return;
    }

    event.preventDefault();
    git.select(reachable[next].entry.path, reachable[next].section);
    buttons[keyOf(reachable[next])]?.focus();
  }
</script>

<div role="group" aria-label={t('changes.files.label')} class="flex flex-col gap-[14px] p-3" data-testid="file-list">
  {#each groups as group (group.section)}
    {@const open = isOpen(group.section)}
    <section class="flex flex-col gap-[6px]">
      <button
        type="button"
        aria-expanded={open}
        disabled={searching}
        class="group/heading flex h-[28px] w-full items-center gap-[10px] rounded-sm px-[12px] text-left text-[11px] leading-[16px] font-medium tracking-wide text-fg-3 uppercase transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg-2 focus-visible:outline-2 focus-visible:outline-offset-[-2px] disabled:hover:bg-transparent disabled:hover:text-fg-3"
        onclick={() => folded.toggle(group.section)}
      >
        <span class="grid size-[16px] shrink-0 place-items-center"><ChevronRight size={14} aria-hidden="true" class={cx('transition-transform duration-[var(--dur-fast)]', open && 'rotate-90')} /></span>
        <span class="flex-1 truncate">{SCOPE_LABELS[group.section]}</span>
        <span class="tabular">{formatCount(group.rows.length)}</span>
      </button>

      {#if open}
        <ul role="listbox" aria-label={SCOPE_LABELS[group.section]} class="flex flex-col gap-[4px]">
          {#each group.rows as row (keyOf(row))}
            {@const entry = row.entry}
            {@const selected = isSelected(row)}
            {@const status = (group.section === 'staged' ? entry.staged : entry.unstaged) ?? entry.staged ?? 'modified'}
            {@const parts = splitPath(entry.path)}
            {@const counts = stats(entry.path, group.section)}
            {@const comments = commentCounts.get(entry.path) ?? 0}
            <li role="presentation">
              <button
                bind:this={buttons[keyOf(row)]}
                type="button"
                role="option"
                aria-selected={selected}
                tabindex={selected ? 0 : -1}
                title={entry.path}
                class={cx(
                  // As in the Extensions list: the focus ring lies on the row's own border, so the scrolling list cannot clip it.
                  'flex w-full items-center gap-2.5 rounded-md border min-h-[36px] px-2.5 py-1 text-left transition-colors duration-[var(--dur-fast)] focus-visible:outline-offset-[-1px]',
                  selected ? 'border-accent/30 bg-accent-wash' : 'border-transparent hover:bg-hover active:bg-press',
                )}
                onclick={() => git.select(entry.path, group.section)}
                onkeydown={(event) => onKeydown(event, row)}
              >
                <span class={cx('grid size-5 shrink-0 place-items-center rounded-sm font-mono text-[11px] font-semibold', TONE_CLASSES[statusTone(status)])} title={statusName(status)}>
                  {statusLetter(status)}<span class="sr-only">{` ${statusName(status)}`}</span>
                </span>

                <span class="flex min-w-0 flex-1 flex-col">
                  <span class="truncate text-sm text-fg"><span class="text-fg-3">{parts.dir}</span>{parts.name}</span>
                  {#if entry.origPath}
                    <span class="truncate text-xs text-fg-3">{entry.origPath} → {entry.path}</span>
                  {/if}
                </span>

                {#if comments > 0}
                  <span class="tabular rounded-full bg-accent-wash px-1.5 text-xs leading-5 text-accent-text" title={tn('changes.files.pending', comments)}>{comments}<span class="sr-only">{` ${tn('changes.files.commentWord', comments)}`}</span></span>
                {/if}

                {#if git.hits.get(entry.path)?.lines}
                  <span class="tabular rounded-full bg-warning-wash px-1.5 text-xs leading-5 text-warning" title={tn('changes.search.hitsIn', git.hits.get(entry.path)?.lines ?? 0)}>{git.hits.get(entry.path)?.lines}<span class="sr-only">{` ${tn('changes.search.hitsIn', git.hits.get(entry.path)?.lines ?? 0)}`}</span></span>
                {/if}

                {#if counts}
                  <span class="tabular shrink-0 text-xs">
                    <span class="text-success">+{counts.additions}</span>
                    <span class="text-danger">−{counts.deletions}</span>
                  </span>
                {/if}
            </button>
          </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/each}
</div>
