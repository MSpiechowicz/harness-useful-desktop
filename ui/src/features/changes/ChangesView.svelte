<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import GitCompare from '@lucide/svelte/icons/git-compare';
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import SearchX from '@lucide/svelte/icons/search-x';
  import { untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import { ICON } from '$lib/components/icon';
  import ListDetail from '$lib/components/ListDetail.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import RefreshButton from '$lib/components/RefreshButton.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
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
      return t('changes.empty.filtered');
    }

    switch (git.scope) {
      case 'all':
        return t('changes.empty.clean');

      case 'unstaged':
        return t('changes.empty.unstaged');

      case 'staged':
        return t('changes.empty.staged');

      case 'untracked':
        return t('changes.empty.untracked');
    }
  });
</script>

<div class="flex size-full min-h-0 flex-col">
  <ViewHeader title={t('changes.title')} icon={GitCompare} context={scopeLabel(git.status?.isRepo ? count(git.status.entries.length, 'changed file') : null)}>
    {#snippet actions()}
      {#if agentSessions.active}
        <Button variant="ghost" onclick={() => nav.go('chat')}>
          {#snippet leading()}<MessageSquare size={ICON.sm} aria-hidden="true" />{/snippet}
          {t('changes.backToChat')}
        </Button>
      {/if}
      <RefreshButton label={t('changes.refresh')} disabled={!active} busy={git.refreshing} onclick={() => void git.refresh()} />
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
      <div class="flex size-full min-h-0 flex-col">
        <ChangesHeader status={git.status} />

        <div class="min-h-0 flex-1 border-t border-line">
          {#if git.entries.length === 0}
            <div class="grid size-full place-items-center">
              <EmptyState icon={SearchX} title={emptyMessage}>
                {#snippet action()}
                  {#if git.filter}<Button onclick={() => git.clearFilter()}>{t('changes.showAll')}</Button>{/if}
                {/snippet}
              </EmptyState>
            </div>
          {:else}
            <ListDetail label={t('changes.resize')} size={300}>
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
