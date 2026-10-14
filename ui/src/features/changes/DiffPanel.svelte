<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import ChevronUp from '@lucide/svelte/icons/chevron-up';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import IconButton from '$lib/components/IconButton.svelte';
  import DiffView from '$lib/components/DiffView.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import type { RangeTarget } from '$lib/git/comment-range';
  import { statusName, statusTone } from '$lib/git/entries';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { StatusEntry } from '$lib/ipc/bindings/StatusEntry';
  import { ipc } from '$lib/ipc/client';
  import { git } from '$lib/stores/git.svelte';
  import { revealPath } from '$lib/ipc/actions';
  import { MAX_TEXT_BYTES, review } from '$lib/stores/review.svelte';

  import { diffMode } from './diff-mode.svelte';

  interface Props {
    entry: StatusEntry;
    project: string;
  }

  let { entry, project }: Props = $props();

  const loaded = $derived(git.diffFor(entry.path));
  const diffs = $derived(loaded?.status === 'ready' ? loaded.diffs : []);

  // A file that is both staged and edited has two diffs; the heading it was picked under says which one to show.
  const staged = $derived(diffs.find((diff) => diff.staged));
  const unstaged = $derived(diffs.find((diff) => !diff.staged));
  const shown = $derived(git.section === 'staged' ? (staged ?? unstaged) : (unstaged ?? staged));

  const comments = $derived(review.commentsFor(project).filter((comment) => comment.path === entry.path));

  function add(target: RangeTarget, body: string): void {
    review.add(project, { path: entry.path, ...target, body });
  }
</script>

<div class="flex size-full min-h-0 flex-col">
  <header class="flex h-[52px] shrink-0 items-center gap-3 border-b border-line px-4">
    <h3 class="min-w-0 shrink truncate font-mono text-sm text-fg" title={entry.path}>{entry.path}</h3>
    {#if shown}
      <Badge tone={statusTone(shown.status)}>{statusName(shown.status)}</Badge>
      {#if !shown.isBinary}
        <span class="tabular text-xs"><span class="text-success">+{shown.additions}</span> <span class="text-danger">−{shown.deletions}</span></span>
      {/if}
    {/if}
    {#if shown?.oldPath}
      <span class="truncate text-xs text-fg-3">{t('changes.diff.renamedFrom', { path: shown.oldPath })}</span>
    {/if}

    {#if git.matchTotal > 0}
      <div class="ml-auto flex shrink-0 items-center gap-1" role="group" aria-label={t('changes.search.matches')}>
        <span class="tabular px-1 text-xs text-fg-2" aria-live="polite">{t('changes.search.position', { current: git.cursor + 1, total: git.matchTotal })}</span>
        <IconButton icon={ChevronUp} label={t('changes.search.previous')} onclick={() => git.stepMatch(-1)} />
        <IconButton icon={ChevronDown} label={t('changes.search.next')} onclick={() => git.stepMatch(1)} />
      </div>
    {:else if review.commentsFor(project).length === 0}
      <p class="ml-auto hidden min-w-0 truncate text-xs text-fg-3 @2xl:block">{t('changes.diff.hint')}</p>
    {/if}
  </header>

  <div class="flex min-h-0 flex-1 flex-col">
    {#if !loaded || loaded.status === 'loading'}
      <div class="flex flex-col gap-2 p-4" role="status" aria-label={t('changes.diff.loading')}>
        <Skeleton width="40%" height="1rem" />
        <Skeleton height="1rem" />
        <Skeleton height="1rem" />
        <Skeleton width="70%" height="1rem" />
      </div>
    {:else if loaded.status === 'error'}
      <div class="flex flex-col items-center gap-3 p-8 text-center" role="alert">
        <CircleAlert size={24} class="text-danger" aria-hidden="true" />
        <p class="text-base text-fg-2">{loaded.error.message}</p>
        <Button onclick={() => git.retryDiff(entry.path)}>{t('changes.tryAgain')}</Button>
      </div>
    {:else if shown}
      <div class="min-h-0 flex-1">
      {#key `${shown.path}:${shown.staged}`}
        <DiffView
          diff={shown}
          mode={diffMode.value}
          query={git.search}
          cursor={git.cursor}
          onmatches={(count) => (git.matchTotal = count)}
          loadLines={shown.isBinary ? undefined : () => ipc.invoke('git_file_lines', { project, path: shown.path, staged: shown.staged })}
          {comments}
          maxBytes={MAX_TEXT_BYTES}
          oncomment={add}
          onedit={(id, body) => review.edit(project, id, body)}
          onremove={(id) => review.remove(project, id)}
          onreveal={() => void revealPath(`${project}/${entry.path}`)}
        />
      {/key}
      </div>
    {:else}
      <p class="grid size-full place-items-center p-8 text-center text-base text-fg-2" role="status">{t('changes.diff.unavailable')}</p>
    {/if}
  </div>
</div>
