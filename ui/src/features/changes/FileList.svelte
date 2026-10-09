<script lang="ts">
  import { displayStatus, statusLetter, statusName, statusTone, type StatusTone } from '$lib/git/entries';
  import type { StatusEntry } from '$lib/ipc/bindings/StatusEntry';
  import { git } from '$lib/stores/git.svelte';
  import { cx } from '$lib/utils/cx';
  import { rovingIndex } from '$lib/utils/roving';

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

  let options: (HTMLElement | undefined)[] = $state([]);

  function split(path: string): { dir: string; name: string } {
    const cut = path.lastIndexOf('/');

    return cut === -1 ? { dir: '', name: path } : { dir: path.slice(0, cut + 1), name: path.slice(cut + 1) };
  }

  /** Added and removed lines of a file, once its diff has loaded. */
  function stats(path: string): { additions: number; deletions: number } | null {
    const state = git.diffFor(path);
    if (state?.status !== 'ready' || state.diffs.length === 0 || state.diffs.every((diff) => diff.isBinary)) {
      return null;
    }

    return {
      additions: state.diffs.reduce((sum, diff) => sum + diff.additions, 0),
      deletions: state.diffs.reduce((sum, diff) => sum + diff.deletions, 0),
    };
  }

  function onKeydown(event: KeyboardEvent, index: number): void {
    const next = rovingIndex(event.key, index, entries.length, 'vertical');
    if (next === null) {
      return;
    }

    event.preventDefault();
    git.select(entries[next].path);
    options[next]?.focus();
  }
</script>

<ul role="listbox" aria-label="Changed files" class="flex flex-col gap-0.5 p-2">
  {#each entries as entry, index (entry.path)}
    {@const status = displayStatus(entry, git.scope)}
    {@const selected = entry.path === git.selected}
    {@const parts = split(entry.path)}
    {@const counts = stats(entry.path)}
    {@const comments = commentCounts.get(entry.path) ?? 0}
    <li role="presentation">
      <button
        bind:this={options[index]}
        type="button"
        role="option"
        aria-selected={selected}
        tabindex={selected ? 0 : -1}
        title={entry.path}
        class={cx('flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left transition-colors duration-[var(--dur-fast)]', selected ? 'bg-accent-wash' : 'hover:bg-hover')}
        onclick={() => git.select(entry.path)}
        onkeydown={(event) => onKeydown(event, index)}
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
          <span class="tabular rounded-full bg-accent-wash px-1.5 text-xs leading-5 text-accent-text" title="{comments} pending {comments === 1 ? 'comment' : 'comments'}">{comments}<span class="sr-only">{' comments'}</span></span>
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
