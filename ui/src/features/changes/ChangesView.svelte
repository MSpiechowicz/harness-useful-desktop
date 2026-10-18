<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import GitCompare from '@lucide/svelte/icons/git-compare';
  import SearchX from '@lucide/svelte/icons/search-x';
  import X from '@lucide/svelte/icons/x';
  import { untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import SearchField from '$lib/components/SearchField.svelte';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import CostChip from '$lib/components/CostChip.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import Columns2 from '@lucide/svelte/icons/columns-2';
  import Rows2 from '@lucide/svelte/icons/rows-2';
  import { ICON } from '$lib/components/icon';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import WorkspaceLayout from '$lib/components/WorkspaceLayout.svelte';
  import WorkspaceToggles from '$lib/components/WorkspaceToggles.svelte';
  import { WorkspaceState } from '$lib/components/workspace.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { review } from '$lib/stores/review.svelte';

  import { count, scopeLabel } from '../shell/scope';

  import type { StatusEntry } from '$lib/ipc/bindings/StatusEntry';
  import ChangesMenu from './ChangesMenu.svelte';
  import DiffPanel from './DiffPanel.svelte';
  import DiscardFileDialog from './DiscardFileDialog.svelte';
  import { diffMode, type DiffMode } from './diff-mode.svelte';
  import FileList from './FileList.svelte';
  import ReviewButton from './ReviewButton.svelte';

  const active = $derived(project.active);
  const costArgs = $derived(git.status?.branch && active ? { branch: git.status.branch, project: active.path } : null);
  let discarding = $state<StatusEntry | null>(null);
  const workspace = new WorkspaceState('changes', { details: false });
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


  const modes = $derived<StripOption<DiffMode>[]>([
    { value: 'unified', label: t('changes.header.unified'), icon: Rows2 },
    { value: 'split', label: t('changes.header.split'), icon: Columns2 },
  ]);

  // The shell re-reads git in the background (see Shell.svelte), so edits made by hand or by an agent show up without
  // asking, and a read that failed is retried there, a little later each time.

  function onSearchKey(event: KeyboardEvent): void {
    if (event.key === 'Enter') {
      event.preventDefault();
      git.stepMatch(event.shiftKey ? -1 : 1);
    } else if (event.key === 'Escape' && git.search !== '') {
      event.preventDefault();
      event.stopPropagation();
      git.setSearch('');
    }
  }

  /** Nothing changed at all: no list to show, so the page says so in full instead of leaving an empty list beside a message. */
  const clean = $derived(git.status?.isRepo === true && git.status.entries.length === 0);

  const emptyMessage = $derived.by(() => {
    if (git.searching && git.search.trim() !== '') {
      return t('changes.search.searching');
    }

    if (git.search.trim() !== '') {
      return t('changes.search.none', { query: git.search.trim() });
    }

    if (git.filter) {
      return t('changes.empty.filtered');
    }

    return t('changes.empty.clean');
  });

</script>

<div class="flex size-full min-h-0 flex-col">
  <ViewHeader title={t('changes.title')} icon={GitCompare} context={scopeLabel(git.status?.isRepo ? count(git.status.entries.length, 'changed file') : null)}>
    {#snippet actions()}
      {#if active && git.status?.isRepo && !clean}
        <ReviewButton project={active.path} />
        <div class="search-width">
          <SearchField bind:value={() => git.search, (query) => git.setSearch(query)} label={t('changes.search.label')} placeholder={t('changes.search.placeholder')} shortcut="mod+f" onkeydown={onSearchKey} />
        </div>
        <SegmentedControl options={modes} value={diffMode.value} label={t('changes.header.layout')} onchange={(mode) => diffMode.set(mode)} />
      {/if}
    {/snippet}

    {#snippet fixed()}
      {#if active && git.status?.isRepo && !clean}
        <WorkspaceToggles {workspace} listLabel={t('changes.drawer.label')} />
        <ChangesMenu project={active.name} file={selected} ondiscardfile={(entry) => (discarding = entry)} />
      {/if}
    {/snippet}
  </ViewHeader>

  <div class="relative min-h-0 flex-1">
    {#if !active}
      <div class="grid size-full place-items-center">
        <EmptyState icon={FolderOpen} title={t('changes.empty.noProject.title')} body={t('changes.empty.noProject.body')}>
          {#snippet action()}
            <Button variant="primary" onclick={() => void project.pick()}>{t('changes.empty.noProject.action')}</Button>
          {/snippet}
        </EmptyState>
      </div>
    {:else if !git.status && git.loadStatus === 'error'}
      <div class="grid size-full place-items-center">
        <EmptyState icon={CircleAlert} title={t('changes.empty.error')} body={git.error?.message}>
          {#snippet action()}
            <Button onclick={() => void git.refresh()}>{t('changes.tryAgain')}</Button>
          {/snippet}
        </EmptyState>
      </div>
    {:else if !git.status}
      <div class="flex flex-col gap-3 p-8" role="status" aria-label={t('changes.loading')}>
        <Skeleton width="14rem" height="1.75rem" />
        <Skeleton height="2.5rem" />
        <Skeleton height="2.5rem" />
      </div>
    {:else if !git.status.isRepo}
      <div class="grid size-full place-items-center">
        <EmptyState icon={GitCompare} title={t('changes.empty.notRepo.title', { name: active.name })} body={t('changes.empty.notRepo.body')} />
      </div>
    {:else}
      {#if clean}
        <div class="grid size-full place-items-center">
          <EmptyState icon={CircleCheck} title={t('changes.empty.clean.title')} body={t('changes.empty.clean.body')}>
            {#snippet action()}
              {#if git.status?.branch}
                <span class="inline-flex h-control items-center gap-2 rounded-md border border-line bg-surface-2 px-3.5 text-sm text-fg-2">
                  <GitBranch size={ICON.sm} aria-hidden="true" />
                  <span class="font-mono">{git.status?.branch}</span>
                </span>
              {/if}
            {/snippet}
          </EmptyState>
        </div>
      {:else}
      <div class="min-h-0 size-full">
        <WorkspaceLayout {workspace} listLabel={t('changes.drawer.label')} resizeLabel={t('changes.resize')} scroll="own">
          {#snippet list()}
            {#if git.filter}
              <div class="pane-header flex items-center border-b border-panel-edge panel-inset">
                <span class="inline-flex h-control min-w-0 items-center gap-1 rounded-full bg-accent-wash pr-1.5 pl-3 text-xs font-medium text-accent-text" data-testid="touched-filter">
                  <span class="truncate">{t('changes.header.touched', { label: git.filter.label, files: count(git.filter.paths.length, 'file') })}</span>
                  <button type="button" aria-label={t('changes.showAll')} class="grid size-7 shrink-0 place-items-center rounded-full hover:bg-hover" onclick={() => git.clearFilter()}>
                    <X size={ICON.sm} aria-hidden="true" />
                  </button>
                </span>
              </div>
            {/if}
            <div class="relative min-h-0 flex-1 overflow-y-auto"><FileList entries={git.entries} {commentCounts} onpick={() => workspace.drawer.close()} /></div>
            {#if costArgs}
              <div class="flex h-10 shrink-0 items-center px-2 empty:hidden">
                <CostChip kind="branch" args={costArgs} align="start" side="top" quiet />
              </div>
            {/if}
          {/snippet}
          {#snippet main()}
            {#if selected}
              {#key selected.path}
                <DiffPanel entry={selected} project={active.path} />
              {/key}
            {:else}
              <div class="grid size-full place-items-center">
                <EmptyState icon={SearchX} title={emptyMessage}>
                  {#snippet action()}
                    {#if git.filter}<Button onclick={() => git.clearFilter()}>{t('changes.showAll')}</Button>{/if}
                  {/snippet}
                </EmptyState>
              </div>
            {/if}
          {/snippet}
        </WorkspaceLayout>
      </div>
      {/if}
    {/if}
  </div>
</div>


<DiscardFileDialog bind:file={discarding} />
