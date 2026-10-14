<script lang="ts">
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import Layers from '@lucide/svelte/icons/layers';
  import PackageOpen from '@lucide/svelte/icons/package-open';
  import PanelLeftOpen from '@lucide/svelte/icons/panel-left-open';
  import { untrack } from 'svelte';
  import { fade, fly } from 'svelte/transition';

  import { focusTrap } from '$lib/a11y/focus-trap';
  import Button from '$lib/components/Button.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import { ICON } from '$lib/components/icon';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import SplitPane from '$lib/components/SplitPane.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import RefreshButton from '$lib/components/RefreshButton.svelte';
  import { scaled } from '$lib/design/motion';
  import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
  import { git } from '$lib/stores/git.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cleanUpCopies, deleteSession } from '$lib/stores/session-actions';
  import { sessionDetail, sessionList } from '$lib/stores/sessions.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { formatBytes, relativeTime } from '$lib/utils/format';

  import { onboarding } from '../onboarding/onboarding-state.svelte';
  import { count, scopeLabel } from '../shell/scope';
  import BranchesPanel from './BranchesPanel.svelte';
  import { sessionTitle } from './session-format';
  import { forkCount, groupSessions } from './session-groups';
  import SessionConfirm, { type ConfirmRequest } from './SessionConfirm.svelte';
  import SessionList from './SessionList.svelte';
  import SessionPane from './SessionPane.svelte';

  /** Below this width the session list moves into a drawer so the session itself keeps its room. */
  const DRAWER_BELOW_PX = 1100;

  let listSize = $state(320);
  let width = $state(0);
  let drawerOpen = $state(false);
  let confirmer: SessionConfirm | undefined = $state();

  const active = $derived(harness.active);
  const current = $derived(project.active);
  const sessions = $derived(sessionList.data ?? []);
  const groups = $derived(groupSessions(sessions));

  // The chosen session, or the newest one that is not a folded empty copy until the user chooses.
  const selected = $derived(sessions.find((summary) => summary.ref.id === viewState.sessionId) ?? groups.rows[0] ?? null);
  const drawer = $derived(width > 0 && width < DRAWER_BELOW_PX);

  // Opening this view (or switching harness or project) re-reads the sessions, so ones started since show up.
  $effect(() => {
    const harnessId = active?.info.id ?? null;
    const path = current?.path ?? null;

    // The stores read their own state while loading; only the harness and project may re-run this.
    untrack(() => {
      sessionList.revalidate(harnessId, path);
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

  function ask(request: ConfirmRequest): Promise<boolean> {
    return confirmer?.ask(request) ?? Promise.resolve(false);
  }

  function remove(summary: SessionSummary): void {
    if (!current) {
      return;
    }

    void deleteSession(agentSessions, summary.ref, current.path, () =>
      ask({
        title: t('sessions.delete.title'),
        description: t('sessions.delete.body'),
        subject: { icon: GitBranch, name: sessionTitle(summary), details: sessionDetails(summary) },
        confirmLabel: t('sessions.delete.confirm'),
      }),
    );
  }

  /** When, on which model and how big: the details that tell sessions with the same first prompt apart. */
  function sessionDetails(summary: SessionSummary): string {
    return [relativeTime(summary.updatedAt), summary.model, formatBytes(summary.sizeBytes)].filter(Boolean).join(' · ');
  }

  /** The copies' session when they all belong to one, else how many sessions they repeat; with their total size. */
  function copiesSubject(copies: SessionSummary[]): ConfirmRequest['subject'] {
    const roots = [...groups.copiesByRoot].filter(([, listed]) => listed.some((copy) => copies.includes(copy))).map(([id]) => id);
    const size = formatBytes(copies.reduce((total, copy) => total + copy.sizeBytes, 0));
    const root = roots.length === 1 ? sessions.find((summary) => summary.ref.id === roots[0]) : undefined;

    if (root) {
      return { icon: Layers, name: sessionTitle(root), details: `${tn('sessions.cleanup.banner', copies.length)} · ${size}` };
    }

    return { icon: Layers, name: tn('sessions.cleanup.banner', copies.length), details: `${tn('sessions.cleanup.ofSessions', roots.length)} · ${size}` };
  }

  function cleanUp(copies: SessionSummary[]): void {
    if (!current) {
      return;
    }

    void cleanUpCopies(agentSessions, copies, current.path, () =>
      ask({
        title: tn('sessions.cleanup.confirmTitle', copies.length),
        description: t('sessions.cleanup.confirmBody'),
        subject: copiesSubject(copies),
        confirmLabel: t('sessions.cleanup.confirm'),
      }),
    );
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
  <ViewHeader title={t('sessions.title')} icon={GitBranch} context={scopeLabel(current && sessionList.data ? count(groups.rows.length, 'session') : null)}>
    {#snippet actions()}
      {#if drawer && current}
        <Button data-drawer-toggle aria-haspopup="dialog" aria-expanded={drawerOpen} onclick={() => (drawerOpen = !drawerOpen)}>
          {#snippet leading()}<PanelLeftOpen size={ICON.sm} aria-hidden="true" />{/snippet}
          {t('sessions.drawer.label')}
          <span class="tabular rounded-full bg-surface-3 px-1.5 text-xs leading-5 text-fg-2">{groups.rows.length}</span>
        </Button>
      {/if}
      <RefreshButton label={t('sessions.refresh')} busy={sessionList.status === 'loading'} onclick={() => sessionList.refresh()} />
    {/snippet}
  </ViewHeader>

  <div class="relative min-h-0 flex-1">
    {#if harness.status === 'idle' || harness.status === 'loading'}
      <div class="flex flex-col gap-3 p-8" role="status" aria-label={t('sessions.loading')}>
        <Skeleton width="14rem" height="1.75rem" />
        <Skeleton height="2.5rem" />
        <Skeleton height="2.5rem" />
      </div>
    {:else if !active || (!active.detection.installed && !active.detection.configDirExists)}
      <div class="grid size-full place-items-center">
        <EmptyState icon={PackageOpen} title={t('sessions.empty.noHarness.title')} body={t('sessions.empty.noHarness.body')}>
          {#snippet action()}
            <Button variant="primary" onclick={() => onboarding.restart()}>{t('sessions.empty.noHarness.action')}</Button>
          {/snippet}
        </EmptyState>
      </div>
    {:else if !current}
      <div class="grid size-full place-items-center">
        <EmptyState icon={FolderOpen} title={t('sessions.empty.noProject.title')} body={t('sessions.empty.noProject.body')}>
          {#snippet action()}
            <Button variant="primary" onclick={() => void project.pick()}>{t('sessions.empty.noProject.action')}</Button>
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
              aria-label={t('sessions.drawer.label')}
              use:focusTrap={{ onEscape: closeDrawer, initialFocus: 'input' }}
              transition:fly={{ x: -32, duration: scaled(240) }}
              class="absolute inset-y-0 left-0 z-40 w-[min(24rem,92%)] border-r border-line-strong bg-surface-1 shadow-pop outline-none"
            >
              {@render listColumn()}
            </div>
          {/if}
        {:else}
          <SplitPane label={t('sessions.resize.list')} bind:size={listSize} min={240} minSecond={480}>
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
        <Button onclick={() => sessionList.refresh()}>{t('sessions.tryAgain')}</Button>
      </div>
    {:else if sessionList.status === 'loading' && !sessionList.data}
      <div class="flex flex-col gap-2 p-3" role="status" aria-label={t('sessions.loading')}>
        <Skeleton height="3.5rem" />
        <Skeleton height="3.5rem" />
        <Skeleton height="3.5rem" />
      </div>
    {:else}
      <SessionList {groups} selectedId={selected?.ref.id ?? null} onselect={choose} ondelete={remove} oncleanup={cleanUp} />
    {/if}

    <BranchesPanel />
  </div>
{/snippet}

{#snippet detail(project: NonNullable<typeof current>, harness: NonNullable<typeof active>)}
  {#if selected}
    {#key selected.ref.id}
      <SessionPane summary={selected} project={project.path} forks={forkCount(sessions, selected.ref.id)} ondelete={() => remove(selected)} />
    {/key}
  {:else}
    <div class="grid size-full place-items-center">
      <EmptyState icon={GitBranch} title={t('sessions.empty.none.title')} body={t('sessions.empty.none.body', { project: project.name, harness: harness.info.name })} />
    </div>
  {/if}
{/snippet}

<SessionConfirm bind:this={confirmer} />
