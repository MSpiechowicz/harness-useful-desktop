<script lang="ts">
  import Archive from '@lucide/svelte/icons/archive';
  import Check from '@lucide/svelte/icons/check';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import { onDestroy, untrack } from 'svelte';

  import Spinner from '$lib/components/Spinner.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { branchSync } from '$lib/stores/branch-sync.svelte';

  import BranchList from './BranchList.svelte';
  import ConflictPanel from './ConflictPanel.svelte';
  import MainPicker from './MainPicker.svelte';
  import MainRow from './MainRow.svelte';
  import Note from './Note.svelte';
  import RemoteRow from './RemoteRow.svelte';
  import SectionLabel from './SectionLabel.svelte';
  import StashNote from './StashNote.svelte';
  import SyncHeader from './SyncHeader.svelte';

  interface Props {
    session: ChatSession;
    /** Closes the popover. */
    close: () => void;
    /** Asks whether to drop the copy of the changes that is left in the stash. */
    ondrop: () => void;
  }

  let { session, close, ondrop }: Props = $props();

  /** `pickMain` is a sub-view of the same panel: a popover inside a popover would be clipped by it. */
  let view = $state<'branch' | 'pickMain'>('branch');

  const sync = $derived(branchSync.state);
  const note = $derived(branchSync.note);

  // Opening the menu looks again at once: the repository, then the remote (when the user allows it and the last look is old enough).
  // `untrack`: the store reads its own state on the way, and a rerun on every update would wipe the note of the first action.
  $effect(() => {
    untrack(() => {
      branchSync.clearNote();
      void branchSync.read();
      void branchSync.fetchRemote();
    });
  });

  onDestroy(() => branchSync.clearNote());
</script>

<div class="flex flex-col">
  {#if sync === null && !branchSync.readFailed}
    <div class="grid h-control place-items-center text-fg-3"><Spinner /></div>
  {:else if sync !== null && view === 'pickMain'}
    <MainPicker {sync} onback={() => (view = 'branch')} />
  {:else if sync === null}
    <!-- The first read failed: say so once, and the branches below still work. -->
    <Note tone="danger" icon={TriangleAlert} title={t('branch.unreadable')} />
    <BranchList sync={null} />
  {:else}
    <SyncHeader branch={sync.branch} remote={sync.remote} lastFetched={sync.lastFetched} failed={branchSync.fetchFailed} />

    {#if sync.operation || sync.unmerged.length > 0}
      <ConflictPanel {sync} operation={sync.operation} {session} {close} />
    {:else if sync.branch !== null && sync.head !== null}
      <!-- Before the first commit and on a detached HEAD there is nothing to push, pull or rebase: the branches below are what is left. -->
      {#if sync.remote || !sync.main?.isCurrent}
        <SectionLabel>{t('branch.sync.title')}</SectionLabel>

        <div class="mx-2.5 mb-2.5 divide-y divide-line rounded-md border border-line bg-surface-2">
          {#if sync.remote}
            <RemoteRow {sync} remote={sync.remote} />
          {/if}

          {#if !sync.main?.isCurrent}
            <MainRow {sync} main={sync.main} onpick={() => (view = 'pickMain')} />
          {/if}
        </div>
      {/if}

      {#if sync.main && !sync.main.isCurrent && sync.trackedChanges > 0}
        <Note tone="warning" icon={Archive} title={tn('branch.dirty', sync.trackedChanges)} />
      {/if}
    {/if}

    <!-- Applying the stash is refused while files still hold conflict markers. -->
    {#if sync.stashKept && sync.unmerged.length === 0}
      <StashNote {sync} {ondrop} />
    {/if}

    {#if note}
      <Note tone={note.tone} icon={note.tone === 'success' ? Check : TriangleAlert} title={note.text} names={note.names} detail={note.detail} />
    {/if}

    <BranchList {sync} />
  {/if}
</div>
