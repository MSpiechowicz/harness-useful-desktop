<script lang="ts">
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import PackageOpen from '@lucide/svelte/icons/package-open';
  import PanelLeftOpen from '@lucide/svelte/icons/panel-left-open';
  import RotateCw from '@lucide/svelte/icons/rotate-cw';
  import { untrack } from 'svelte';
  import { fade, fly } from 'svelte/transition';

  import { focusTrap } from '$lib/a11y/focus-trap';
  import Button from '$lib/components/Button.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import { ICON } from '$lib/components/icon';
  import IconButton from '$lib/components/IconButton.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import SplitPane from '$lib/components/SplitPane.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import { scaled } from '$lib/design/motion';
  import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
  import { git } from '$lib/stores/git.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { sessionDetail, sessionList } from '$lib/stores/sessions.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';

  import { onboarding } from '../onboarding/onboarding.svelte';
  import { count, scopeLabel } from '../shell/scope';
  import BranchesPanel from './BranchesPanel.svelte';
  import SessionList from './SessionList.svelte';
  import SessionPane from './SessionPane.svelte';

  /** Below this width the session list moves into a drawer so the session itself keeps its room. */
  const DRAWER_BELOW_PX = 1100;

  let listSize = $state(320);
  let width = $state(0);
  let drawerOpen = $state(false);

  const active = $derived(harness.active);
  const current = $derived(project.active);
  const sessions = $derived(sessionList.data ?? []);

  // The chosen session, or the newest one until the user chooses.
  const selected = $derived(sessions.find((summary) => summary.ref.id === viewState.sessionId) ?? sessions[0] ?? null);
  const drawer = $derived(width > 0 && width < DRAWER_BELOW_PX);

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
    viewState.sessionId = summary.ref.id;
    drawerOpen = false;
  }

  function closeDrawer(): void {
    drawerOpen = false;
    document.querySelector<HTMLElement>('[data-drawer-toggle]')?.focus();
  }

  // Widening the window past the breakpoint turns the drawer back into a column.
  $effect(() => {
    if (!drawer) {
      drawerOpen = false;
    }
  });
</script>

<div class="flex size-full min-h-0 flex-col">
  <ViewHeader title="Sessions" icon={GitBranch} context={scopeLabel(current && sessionList.data ? count(sessions.length, 'session') : null)}>
    {#snippet actions()}
      {#if drawer && current}
        <Button data-drawer-toggle aria-haspopup="dialog" aria-expanded={drawerOpen} onclick={() => (drawerOpen = !drawerOpen)}>
          {#snippet leading()}<PanelLeftOpen size={ICON.sm} aria-hidden="true" />{/snippet}
          Sessions
          <span class="tabular rounded-full bg-surface-3 px-1.5 text-xs leading-5 text-fg-2">{sessions.length}</span>
        </Button>
      {/if}
      <IconButton icon={RotateCw} label="Refresh sessions" side="bottom" disabled={sessionList.status === 'loading'} onclick={() => sessionList.refresh()} />
    {/snippet}
  </ViewHeader>

  <div class="relative min-h-0 flex-1">
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
      <div bind:clientWidth={width} class="relative size-full min-h-0">
        {#if drawer}
          {@render detail(current, active)}

          {#if drawerOpen}
            <button type="button" tabindex="-1" aria-hidden="true" class="absolute inset-0 z-30 cursor-default bg-scrim backdrop-blur-sm" transition:fade={{ duration: scaled(160) }} onclick={closeDrawer}></button>

            <div
              role="dialog"
              aria-label="Sessions"
              use:focusTrap={{ onEscape: closeDrawer, initialFocus: 'input' }}
              transition:fly={{ x: -32, duration: scaled(240) }}
              class="absolute inset-y-0 left-0 z-40 w-[min(24rem,92%)] border-r border-line-strong bg-surface-1 shadow-pop outline-none"
            >
              {@render listColumn()}
            </div>
          {/if}
        {:else}
          <SplitPane label="Resize session list" bind:size={listSize} min={240} minSecond={480}>
            {#snippet first()}
              {@render listColumn()}
            {/snippet}

            {#snippet second()}
              {@render detail(current, active)}
            {/snippet}
          </SplitPane>
        {/if}
      </div>
    {/if}
  </div>
</div>

{#snippet listColumn()}
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
      <SessionList {sessions} selectedId={selected?.ref.id ?? null} onselect={choose} />
    {/if}

    <BranchesPanel />
  </div>
{/snippet}

{#snippet detail(project: NonNullable<typeof current>, harness: NonNullable<typeof active>)}
  {#if selected}
    {#key selected.ref.id}
      <SessionPane summary={selected} project={project.path} />
    {/key}
  {:else}
    <div class="grid size-full place-items-center">
      <EmptyState icon={GitBranch} title="No sessions yet" body="Sessions you run in {project.name} with {harness.info.name} appear here, with their branch tree." />
    </div>
  {/if}
{/snippet}
