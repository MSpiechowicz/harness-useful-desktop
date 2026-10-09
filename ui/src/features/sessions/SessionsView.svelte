<script lang="ts">
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import PackageOpen from '@lucide/svelte/icons/package-open';
  import { untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import SplitPane from '$lib/components/SplitPane.svelte';
  import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
  import { git } from '$lib/stores/git.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { sessionDetail, sessionList } from '$lib/stores/sessions.svelte';

  import { onboarding } from '../onboarding/onboarding.svelte';
  import BranchesPanel from './BranchesPanel.svelte';
  import SessionList from './SessionList.svelte';
  import SessionPane from './SessionPane.svelte';

  let listSize = $state(320);
  let pickedId = $state<string | null>(null);

  const active = $derived(harness.active);
  const current = $derived(project.active);
  const sessions = $derived(sessionList.data ?? []);

  // The chosen session, or the newest one until the user chooses.
  const selected = $derived(sessions.find((summary) => summary.ref.id === pickedId) ?? sessions[0] ?? null);

  // Reload whenever the harness or project changes; the loader ignores a pair it already holds.
  $effect(() => {
    const harnessId = active?.info.id ?? null;
    const path = current?.path ?? null;

    // The stores read their own state while loading; only the harness and project may re-run this.
    untrack(() => {
      sessionList.sync(harnessId, path);
      git.activate(path);
    });
  });

  $effect(() => {
    const ref = selected?.ref ?? null;
    const path = current?.path ?? null;

    untrack(() => sessionDetail.open(ref, path));
  });

  function choose(summary: SessionSummary): void {
    pickedId = summary.ref.id;
  }
</script>

{#if harness.status === 'idle' || harness.status === 'loading'}
  <div class="flex flex-col gap-3 p-8" role="status" aria-label="Loading sessions">
    <Skeleton width="14rem" height="1.75rem" />
    <Skeleton height="2.5rem" />
    <Skeleton height="2.5rem" />
  </div>
{:else if !active || (!active.detection.installed && !active.detection.configDirExists)}
  <div class="grid size-full place-items-center">
    <EmptyState icon={PackageOpen} title="Install a harness to browse sessions" body="Sessions are read from OMP or PI. Run the setup to detect the one you use.">
      {#snippet action()}
        <Button variant="primary" onclick={() => onboarding.restart()}>Set up a harness</Button>
      {/snippet}
    </EmptyState>
  </div>
{:else if !current}
  <div class="grid size-full place-items-center">
    <EmptyState icon={FolderOpen} title="Open a project to see its sessions" body="Sessions are saved per project folder. Pick the project whose history you want to browse.">
      {#snippet action()}
        <Button variant="primary" onclick={() => void project.pick()}>Open a project…</Button>
      {/snippet}
    </EmptyState>
  </div>
{:else}
  <div class="size-full min-h-0">
    <SplitPane label="Resize session list" bind:size={listSize} min={240} minSecond={480}>
      {#snippet first()}
        <div class="flex size-full min-h-0 flex-col">
          {#if sessionList.status === 'error'}
            <div class="p-4">
              <p class="mb-3 text-sm text-danger" role="alert">{sessionList.error?.message}</p>
              <Button onclick={() => sessionList.refresh()}>Try again</Button>
            </div>
          {:else if sessionList.status === 'loading' && !sessionList.data}
            <div class="flex flex-col gap-2 p-3" role="status" aria-label="Loading sessions">
              <Skeleton height="3.5rem" />
              <Skeleton height="3.5rem" />
              <Skeleton height="3.5rem" />
            </div>
          {:else}
            <SessionList {sessions} selectedId={selected?.ref.id ?? null} onselect={choose} onrefresh={() => sessionList.refresh()} refreshing={sessionList.status === 'loading'} />
          {/if}

          <BranchesPanel />
        </div>
      {/snippet}

      {#snippet second()}
        {#if selected}
          {#key selected.ref.id}
            <SessionPane summary={selected} project={current.path} />
          {/key}
        {:else}
          <div class="grid size-full place-items-center">
            <EmptyState icon={GitBranch} title="No sessions yet" body="Sessions you run in {current.name} with {active.info.name} appear here, with their branch tree." />
          </div>
        {/if}
      {/snippet}
    </SplitPane>
  </div>
{/if}
