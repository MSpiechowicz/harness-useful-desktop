<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import GitCompare from '@lucide/svelte/icons/git-compare';
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import RotateCw from '@lucide/svelte/icons/rotate-cw';
  import SearchX from '@lucide/svelte/icons/search-x';
  import { untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import { ICON } from '$lib/components/icon';
  import IconButton from '$lib/components/IconButton.svelte';
  import ListDetail from '$lib/components/ListDetail.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import { SCOPE_LABELS } from '$lib/git/entries';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { review } from '$lib/stores/review.svelte';

  import { count, scopeLabel } from '../shell/scope';

  import ChangesHeader from './ChangesHeader.svelte';
  import DiffPanel from './DiffPanel.svelte';
  import FileList from './FileList.svelte';
  import ReviewTray from './ReviewTray.svelte';

  const active = $derived(project.active);
  const selected = $derived(git.entries.find((entry) => entry.path === git.selected));

  // Switching to this view (or project) re-reads git, so edits the agent just made are visible.
  $effect(() => {
    const path = active?.path ?? null;

    // The store reads its own state while loading; only the project may re-run this.
    untrack(() => git.activate(path));
  });

  const commentCounts = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const comment of review.commentsFor(active?.path ?? null)) {
      counts.set(comment.path, (counts.get(comment.path) ?? 0) + 1);
    }

    return counts;
  });

  const emptyMessage = $derived.by(() => {
    if (git.filter) {
      return 'None of the files this session touched have uncommitted changes.';
    }

    return git.scope === 'all' ? 'The working tree is clean.' : `No ${SCOPE_LABELS[git.scope].toLowerCase()} changes.`;
  });
</script>

<div class="flex size-full min-h-0 flex-col">
  <ViewHeader title="Changes" icon={GitCompare} context={scopeLabel(git.status?.isRepo ? count(git.status.entries.length, 'changed file') : null)}>
    {#snippet actions()}
      {#if agentSessions.active}
        <Button variant="ghost" onclick={() => nav.go('chat')}>
          {#snippet leading()}<MessageSquare size={ICON.sm} aria-hidden="true" />{/snippet}
          Back to chat
        </Button>
      {/if}
      <IconButton icon={RotateCw} label="Refresh changes" side="bottom" disabled={!active || git.refreshing} onclick={() => void git.refresh()} />
    {/snippet}
  </ViewHeader>

  <div class="relative min-h-0 flex-1">
    {#if !active}
      <div class="grid size-full place-items-center">
        <EmptyState icon={FolderOpen} title="Open a project to see its changes" body="Changes shows the git diff of one project folder.">
          {#snippet action()}
            <Button variant="primary" onclick={() => void project.pick()}>Open a project…</Button>
          {/snippet}
        </EmptyState>
      </div>
    {:else if !git.status && git.loadStatus === 'error'}
      <div class="grid size-full place-items-center">
        <EmptyState icon={CircleAlert} title="Could not read the changes" body={git.error?.message}>
          {#snippet action()}
            <Button onclick={() => void git.refresh()}>Try again</Button>
          {/snippet}
        </EmptyState>
      </div>
    {:else if !git.status}
      <div class="flex flex-col gap-3 p-8" role="status" aria-label="Loading changes">
        <Skeleton width="14rem" height="1.75rem" />
        <Skeleton height="2.5rem" />
        <Skeleton height="2.5rem" />
      </div>
    {:else if !git.status.isRepo}
      <div class="grid size-full place-items-center">
        <EmptyState icon={GitCompare} title="{active.name} is not a git repository" body="Run git init in the folder, or open a project that is tracked by git." />
      </div>
    {:else}
      <div class="flex size-full min-h-0 flex-col">
        <ChangesHeader status={git.status} />

        <div class="min-h-0 flex-1 border-t border-line">
          {#if git.entries.length === 0}
            <div class="grid size-full place-items-center">
              <EmptyState icon={SearchX} title={emptyMessage}>
                {#snippet action()}
                  {#if git.filter}<Button onclick={() => git.clearFilter()}>Show all changes</Button>{/if}
                {/snippet}
              </EmptyState>
            </div>
          {:else}
            <ListDetail label="Resize file list" size={300}>
              {#snippet list()}
                <div class="min-h-0 flex-1 overflow-y-auto"><FileList entries={git.entries} {commentCounts} /></div>
              {/snippet}
              {#snippet detail()}
                {#if selected}
                  {#key selected.path}
                    <DiffPanel entry={selected} project={active.path} />
                  {/key}
                {/if}
              {/snippet}
            </ListDetail>
          {/if}
        </div>

        <ReviewTray project={active.path} />
      </div>
    {/if}
  </div>
</div>
