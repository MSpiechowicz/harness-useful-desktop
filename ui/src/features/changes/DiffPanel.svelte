<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import DiffView from '$lib/components/DiffView.svelte';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import type { RangeTarget } from '$lib/git/comment-range';
  import { statusName, statusTone } from '$lib/git/entries';
  import type { StatusEntry } from '$lib/ipc/bindings/StatusEntry';
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

  // A file that is both staged and modified has two diffs; one is shown at a time.
  let which = $state<'staged' | 'unstaged'>('unstaged');
  const staged = $derived(diffs.find((diff) => diff.staged));
  const unstaged = $derived(diffs.find((diff) => !diff.staged));
  const shown = $derived.by(() => {
    if (diffs.length <= 1) {
      return diffs[0];
    }

    return which === 'staged' ? staged : unstaged;
  });

  const comments = $derived(review.commentsFor(project).filter((comment) => comment.path === entry.path));

  function add(target: RangeTarget, body: string): void {
    review.add(project, { path: entry.path, ...target, body });
  }
</script>

<div class="flex size-full min-h-0 flex-col">
  <header class="flex flex-wrap items-center gap-x-3 gap-y-1 border-b border-line px-4 py-2.5">
    <h3 class="min-w-0 truncate font-mono text-sm text-fg" title={entry.path}>{entry.path}</h3>
    {#if shown}
      <Badge tone={statusTone(shown.status)}>{statusName(shown.status)}</Badge>
      {#if !shown.isBinary}
        <span class="tabular text-xs"><span class="text-success">+{shown.additions}</span> <span class="text-danger">−{shown.deletions}</span></span>
      {/if}
    {/if}
    {#if shown?.oldPath}
      <span class="truncate text-xs text-fg-3">renamed from {shown.oldPath}</span>
    {/if}

    {#if diffs.length > 1}
      <SegmentedControl
        options={[
          { value: 'unstaged', label: 'Unstaged' },
          { value: 'staged', label: 'Staged' },
        ]}
        value={which}
        label="Which version of the file"
        onchange={(value) => (which = value)}
      />
    {/if}
  </header>

  <div class="min-h-0 flex-1">
    {#if !loaded || loaded.status === 'loading'}
      <div class="flex flex-col gap-2 p-4" role="status" aria-label="Loading diff">
        <Skeleton width="40%" height="1rem" />
        <Skeleton height="1rem" />
        <Skeleton height="1rem" />
        <Skeleton width="70%" height="1rem" />
      </div>
    {:else if loaded.status === 'error'}
      <div class="flex flex-col items-center gap-3 p-8 text-center" role="alert">
        <CircleAlert size={24} class="text-danger" aria-hidden="true" />
        <p class="text-base text-fg-2">{loaded.error.message}</p>
        <Button onclick={() => git.retryDiff(entry.path)}>Try again</Button>
      </div>
    {:else if shown}
      {#key `${shown.path}:${shown.staged}`}
        <DiffView
          diff={shown}
          mode={diffMode.value}
          {comments}
          maxBytes={MAX_TEXT_BYTES}
          oncomment={add}
          onedit={(id, body) => review.edit(project, id, body)}
          onremove={(id) => review.remove(project, id)}
          onreveal={() => void revealPath(`${project}/${entry.path}`)}
        />
      {/key}
    {:else}
      <p class="grid size-full place-items-center p-8 text-center text-base text-fg-2" role="status">No diff is available for this file.</p>
    {/if}
  </div>
</div>
