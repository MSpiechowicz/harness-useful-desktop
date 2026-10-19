<script lang="ts">
  import CalendarClock from '@lucide/svelte/icons/calendar-clock';
  import Folder from '@lucide/svelte/icons/folder';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import Layers from '@lucide/svelte/icons/layers';
  import PackageOpen from '@lucide/svelte/icons/package-open';
  import MessageSquarePlus from '@lucide/svelte/icons/message-square-plus';
  import Play from '@lucide/svelte/icons/play';
  import { untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import FilterMenu from '$lib/components/FilterMenu.svelte';
  import { ICON } from '$lib/components/icon';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import WorkspaceLayout from '$lib/components/WorkspaceLayout.svelte';
  import WorkspaceToggles from '$lib/components/WorkspaceToggles.svelte';
  import { WorkspaceState } from '$lib/components/workspace.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
  import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';
  import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
  import { ipc } from '$lib/ipc/client';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { cleanUpCopies, deleteSession, resumeInChat } from '$lib/stores/session-actions';
  import { SESSIONS_POLL_MS } from '$lib/stores/live';
  import { routineSessionList, sessionDetail, sessionList } from '$lib/stores/sessions.svelte';
  import SearchField from '$lib/components/SearchField.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { autoRefresh } from '$lib/utils/auto-refresh';
  import { formatBytes, relativeTime } from '$lib/utils/format';

  import { onboarding } from '../onboarding/onboarding-state.svelte';
  import { openLiveRoutineSession } from '../routines/routine-request';
  import { routines } from '../routines/routines-data.svelte';
  import { count, scopeLabel } from '../shell/scope';
  import { sessionTitle } from './session-format';
  import { forkCount, groupSessions } from './session-groups';
  import SessionMenu from './SessionMenu.svelte';
  import SessionConfirm, { type ConfirmRequest } from './SessionConfirm.svelte';
  import SessionDetails from './SessionDetails.svelte';
  import SessionHeader from './SessionHeader.svelte';
  import SessionList from './SessionList.svelte';
  import SessionPane from './SessionPane.svelte';

  const workspace = new WorkspaceState('sessions', { details: true });
  let confirmer: SessionConfirm | undefined = $state();

  const active = $derived(harness.active);
  const current = $derived(project.active);
  // Global routines keep their sessions in a folder of their own, offered while it holds any.
  const routinesFolder = $derived(routines.workspace !== null && (routineSessionList.data?.length ?? 0) > 0 ? routines.workspace : null);
  // A run opened its session there: the folder is shown as soon as the routines' folder is known, whatever the list of it holds yet.
  const inRoutines = $derived(viewState.sessionsFolder === 'routines' && routines.workspace !== null);
  const awaitingRoutines = $derived(viewState.sessionsFolder === 'routines' && routines.workspace === null);
  /** The folder whose sessions are listed: the project, or the routines' own. */
  const folder = $derived(inRoutines && routines.workspace ? { path: routines.workspace, name: t('sessions.folder.routines') } : current ? { path: current.path, name: current.name } : null);
  const folderOptions = $derived([
    { value: 'project' as const, label: current?.name ?? t('sessions.folder.project'), icon: Folder },
    { value: 'routines' as const, label: t('sessions.folder.routines'), icon: CalendarClock },
  ]);
  const sessions = $derived(sessionList.data ?? []);
  const groups = $derived(groupSessions(sessions));

  // The chosen session, or the newest one that is not a folded empty copy until the user chooses. Once shown, a session
  // stays shown: one that turns up later (started in a terminal) is listed but does not take the transcript away.
  // A session asked for by something else (a routine run opened it) is never replaced by another: it is waited for, then said missing.
  let shownId = $state<string | null>(null);
  const asked = $derived(viewState.sessionAsked && viewState.sessionId !== null);
  const found = $derived(sessions.find((summary) => summary.ref.id === viewState.sessionId) ?? null);
  const selected = $derived(found ?? (asked ? null : (sessions.find((summary) => summary.ref.id === shownId) ?? groups.rows[0] ?? null)));
  const listed = $derived(sessionList.status === 'ready' && sessionList.holds(active?.info.id ?? null, folder?.path ?? null));
  const missing = $derived(asked && found === null && listed);
  const looking = $derived(asked && found === null && !listed && sessionList.status !== 'error');
  $effect(() => {
    shownId = selected?.ref.id ?? null;
  });
  // Once the asked-for session is shown, it is the user's choice like any other.
  $effect(() => {
    if (asked && found) {
      viewState.sessionAsked = false;
    }
  });
  // Nothing saved for this project: one page with the way forward, not an empty list beside a message.
  const none = $derived(sessionList.status !== 'error' && sessionList.data != null && sessions.length === 0);

  // Opening this view (or switching harness or project) re-reads the sessions, so ones started since show up.
  $effect(() => {
    const harnessId = active?.info.id ?? null;
    const path = folder?.path ?? null;

    // The stores read their own state while loading; only the harness and project may re-run this.
    untrack(() => {
      sessionList.revalidate(harnessId, path);
    });
  });

  // A session of a global routine's run was asked for before the routines' folder was known: read it, and go back to the project when no global routine ever ran.
  $effect(() => {
    if (awaitingRoutines) {
      untrack(() =>
        routines.readWorkspace().then(() => {
          if (routines.workspace === null) {
            viewState.sessionsFolder = 'project';
          }
        }),
      );
    }
  });

  // Whether the routines' folder holds sessions of this harness decides if it is offered.
  $effect(() => {
    const harnessId = active?.info.id ?? null;
    const path = routines.workspace;

    untrack(() => routineSessionList.revalidate(path === null ? null : harnessId, path));
  });

  $effect(() => {
    const ref = selected?.ref ?? null;
    const path = folder?.path ?? null;

    untrack(() => sessionDetail.open(ref, path));
  });

  // Sessions started or written elsewhere (a terminal, an agent) show up on their own, and so does the end of the open one.
  // A list or a transcript that failed to load is read again on the retry schedule, a little later each time.
  $effect(() =>
    autoRefresh(() => Promise.all([sessionList.refresh({ quiet: true }), routineSessionList.refresh({ quiet: true }), sessionDetail.follow()]), SESSIONS_POLL_MS, {
      failing: sessionList.status === 'error' || sessionDetail.transcriptStatus === 'error',
    }),
  );

  let resuming = $state(false);

  async function resume(summary: SessionSummary, path: string): Promise<void> {
    resuming = true;
    // A routine whose run is still going on is looked at live, resuming its session would fail: the run holds it.
    if (!(await openLiveRoutineSession(summary))) {
      // Each global routine works in a folder of its own below the routines' folder: its session resumes there.
      await resumeInChat(agentSessions, summary.ref, inRoutines ? summary.cwd : path);
    }
    resuming = false;
  }

  /** What an empty list says: where its sessions come from, and what can be done with them (resumed, or only read). */
  function noneBody(projectName: string, owner: HarnessSummary): string {
    const key = owner.capabilities.chat ? 'sessions.empty.none.body' : 'sessions.empty.none.bodyReadOnly';

    return t(key, { project: projectName, harness: owner.info.name });
  }

  function choose(summary: SessionSummary): void {
    viewState.sessionId = summary.ref.id;
    viewState.sessionAsked = false;
    workspace.drawer.close();
  }

  function ask(request: ConfirmRequest): Promise<boolean> {
    return confirmer?.ask(request) ?? Promise.resolve(false);
  }

  function remove(summary: SessionSummary): void {
    if (!folder) {
      return;
    }

    const path = folder.path;

    void deleteSession(agentSessions, summary.ref, path, async () =>
      ask({
        title: t('sessions.delete.title'),
        description: t('sessions.delete.body'),
        subject: { icon: GitBranch, name: sessionTitle(summary), details: sessionDetails(summary), note: await folderNote(summary.ref.harness, path) },
        confirmLabel: t('sessions.delete.confirm'),
      }),
    );
  }

  /**
   * A warning when the project's own settings chose where its sessions live (PI `sessionDir`), so a delete can never
   * reach another folder unnoticed. Nothing when the folder is the harness's own or cannot be read.
   */
  async function folderNote(harnessId: HarnessId, project: string): Promise<string | undefined> {
    const folder = await ipc.invoke('sessions_folder', { harness: harnessId, project }).catch(() => null);

    return folder?.configured ? t('sessions.delete.configuredFolder', { path: folder.path }) : undefined;
  }

  /** When, on which model and how big: the details that tell sessions with the same first prompt apart. */
  function sessionDetails(summary: SessionSummary): string {
    return [relativeTime(summary.updatedAt), summary.model, formatBytes(summary.sizeBytes)].filter(Boolean).join(' · ');
  }

  /** The copies' session when they all belong to one, else how many sessions they repeat; with their total size. */
  function copiesSubject(copies: SessionSummary[]): NonNullable<ConfirmRequest['subject']> {
    const roots = [...groups.copiesByRoot].filter(([, listed]) => listed.some((copy) => copies.includes(copy))).map(([id]) => id);
    const size = formatBytes(copies.reduce((total, copy) => total + copy.sizeBytes, 0));
    const root = roots.length === 1 ? sessions.find((summary) => summary.ref.id === roots[0]) : undefined;

    if (root) {
      return { icon: Layers, name: sessionTitle(root), details: `${tn('sessions.cleanup.banner', copies.length)} · ${size}` };
    }

    return { icon: Layers, name: tn('sessions.cleanup.banner', copies.length), details: `${tn('sessions.cleanup.ofSessions', roots.length)} · ${size}` };
  }

  function cleanUp(copies: SessionSummary[]): void {
    if (!folder) {
      return;
    }

    const path = folder.path;

    void cleanUpCopies(agentSessions, copies, path, async () =>
      ask({
        title: tn('sessions.cleanup.confirmTitle', copies.length),
        description: t('sessions.cleanup.confirmBody'),
        subject: { ...copiesSubject(copies), note: copies[0] ? await folderNote(copies[0].ref.harness, path) : undefined },
        confirmLabel: t('sessions.cleanup.confirm'),
      }),
    );
  }

</script>

<div class="flex size-full min-h-0 flex-col">
  <ViewHeader title={t('sessions.title')} icon={GitBranch} context={scopeLabel(folder && sessionList.data ? count(groups.rows.length, 'session') : null)}>
    {#snippet actions()}
      {#if routinesFolder !== null || inRoutines}
        <FilterMenu
          label={t('sessions.folder.label')}
          prefix={t('sessions.folder.label')}
          wide
          options={folderOptions}
          value={inRoutines ? 'routines' : 'project'}
          onchange={(next) => ((viewState.sessionsFolder = next), (viewState.sessionId = null), (viewState.sessionAsked = false))}
        />
      {/if}
      {#if folder && sessionList.data && !none}
        <div class="search-width">
          <SearchField bind:value={viewState.sessionsQuery} label={t('sessions.list.search')} placeholder={t('sessions.list.searchPlaceholder')} shortcut="mod+f" />
        </div>
      {/if}
      <!-- Resuming starts a chat, which a harness without chat cannot do yet: reading and deleting stay. -->
      {#if selected && folder && harness.canChat(selected.ref.harness)}
        <Button variant="primary" loading={resuming} onclick={() => void resume(selected, folder.path)}>
          {#snippet leading()}<Play size={ICON.sm} aria-hidden="true" />{/snippet}
          {t('sessions.header.resume')}
        </Button>
      {/if}
    {/snippet}

    {#snippet fixed()}
      <WorkspaceToggles {workspace} listLabel={t('sessions.drawer.label')} />
      {#if selected && folder}
        <SessionMenu label={t('sessions.header.menu')} ondelete={() => remove(selected)} />
      {/if}
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
    {:else if awaitingRoutines}
      <div class="flex flex-col gap-3 p-8" role="status" aria-label={t('sessions.loading')}>
        <Skeleton width="14rem" height="1.75rem" />
        <Skeleton height="2.5rem" />
      </div>
    {:else if !folder}
      <div class="grid size-full place-items-center">
        <EmptyState icon={FolderOpen} title={t('sessions.empty.noProject.title')} body={t('sessions.empty.noProject.body')}>
          {#snippet action()}
            <Button variant="primary" onclick={() => void project.pick()}>{t('sessions.empty.noProject.action')}</Button>
          {/snippet}
        </EmptyState>
      </div>
    {:else if none}
      <div class="grid size-full place-items-center">
        <EmptyState icon={GitBranch} title={t('sessions.empty.none.title')} body={noneBody(folder.name, active)}>
          {#snippet action()}
            {#if active.capabilities.chat}
              <Button variant="primary" onclick={() => nav.go('chat')}>
                {#snippet leading()}<MessageSquarePlus size={ICON.sm} aria-hidden="true" />{/snippet}
                {t('sessions.empty.none.action')}
              </Button>
            {/if}
          {/snippet}
        </EmptyState>
      </div>
    {:else}
      <WorkspaceLayout
        {workspace}
        listLabel={t('sessions.drawer.label')}
        resizeLabel={t('sessions.resize.list')}
        scroll="own"
        detailsSubtitle={selected ? sessionTitle(selected) : undefined}
        detailsEmpty={selected ? undefined : { icon: GitBranch, message: t('sessions.details.empty') }}
      >
        {#snippet list()}{@render listColumn()}{/snippet}
        {#snippet header()}
          {#if selected}<SessionHeader summary={selected} />{/if}
        {/snippet}
        {#snippet main()}{@render detail(folder, active)}{/snippet}
        {#snippet details()}{@render sessionFacts()}{/snippet}
      </WorkspaceLayout>
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
  </div>
{/snippet}

{#snippet detail(project: NonNullable<typeof folder>, harness: NonNullable<typeof active>)}
  {#if selected}
    {#key selected.ref.id}
      <SessionPane summary={selected} project={project.path} {resuming} />
    {/key}
  {:else if looking}
    <div class="flex flex-col gap-3 p-8" role="status" aria-label={t('sessions.loading')}>
      <Skeleton width="14rem" height="1.75rem" />
      <Skeleton height="2.5rem" />
    </div>
  {:else if missing}
    <div class="grid min-h-0 flex-1 place-items-center">
      <EmptyState icon={GitBranch} title={t('sessions.missing.title')} body={t('sessions.missing.body')} />
    </div>
  {:else}
    <div class="grid min-h-0 flex-1 place-items-center">
      <EmptyState icon={GitBranch} title={t('sessions.empty.none.title')} body={noneBody(project.name, harness)} />
    </div>
  {/if}
{/snippet}

{#snippet sessionFacts()}
  {#if selected && folder}
    <SessionDetails
      summary={selected}
      stats={sessionDetail.stats}
      touched={sessionDetail.touched}
      project={folder.path}
      forks={forkCount(sessions, selected.ref.id)}
      showForks={harness.get(selected.ref.harness)?.capabilities.sessionTree ?? true}
    />
  {/if}
{/snippet}

<SessionConfirm bind:this={confirmer} />
