<script lang="ts">
  import CalendarClock from '@lucide/svelte/icons/calendar-clock';
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import Clock from '@lucide/svelte/icons/clock';
  import History from '@lucide/svelte/icons/history';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import UserMinus from '@lucide/svelte/icons/user-minus';
  import UsersRound from '@lucide/svelte/icons/users-round';
  import { untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import DocFilter from '$lib/components/docs/DocFilter.svelte';
  import DocNewButton from '$lib/components/docs/DocNewButton.svelte';
  import type { DocTableHandle } from '$lib/components/docs/doc-groups';
  import { closePanel, emptyReason, emptySearchOnEscape } from '$lib/components/docs/doc-view';
  import DocMcpHint from '$lib/components/docs/DocMcpHint.svelte';
  import DocSplit from '$lib/components/docs/DocSplit.svelte';
  import DocStage from '$lib/components/docs/DocStage.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import FilterMenu from '$lib/components/FilterMenu.svelte';
  import FilterRow from '$lib/components/FilterRow.svelte';
  import { ICON, type IconComponent } from '$lib/components/icon';
  import MenuItem from '$lib/components/MenuItem.svelte';
  import MoreMenu from '$lib/components/MoreMenu.svelte';
  import SearchField from '$lib/components/SearchField.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import { rise } from '$lib/design/motion';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { RoutineView } from '$lib/ipc/bindings/RoutineView';
  import { ROUTINES_POLL_MS } from '$lib/stores/live';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';
  import { type RoutineSource, type RoutinesSection, type RoutineStatusFilter, type RoutineWhenFilter, viewState } from '$lib/stores/view-state.svelte';
  import { autoRefresh } from '$lib/utils/auto-refresh';

  import { onboarding } from '../onboarding/onboarding-state.svelte';
  import SessionConfirm, { type ConfirmRequest } from '../sessions/SessionConfirm.svelte';
  import DeleteRoutinesDialog from './DeleteRoutinesDialog.svelte';
  import { openRunChat, openRunSession } from './routine-open';
  import { filterCounts, folderName, groupRoutines, inSource, inStatus, inWhen, matchesQuery, routineTitle, sortRoutines, STATUS_LABELS, STATUSES, viewKey, WHEN_ICONS, WHEN_KINDS, WHEN_LABELS, whenText } from './routine-format';
  import type { EditorTarget } from './routine-form';
  import { NO_HANDLERS, type RoutineHandlers } from './routine-handlers';
  import { turnOnTray } from './routine-tray';
  import { originalOf } from './routine-team';
  import RoutineEditor from './RoutineEditor.svelte';
  import RoutineList from './RoutineList.svelte';
  import RoutinePanel from './RoutinePanel.svelte';
  import { routines } from './routines-data.svelte';
  import RunHistory from './RunHistory.svelte';
  import TrayHint from './TrayHint.svelte';

  let confirmer: SessionConfirm | undefined = $state();
  let table: DocTableHandle | undefined = $state();
  let editorOpen = $state(false);
  let editorTarget = $state<EditorTarget>({ view: null, scope: 'global' });
  let deleteOpen = $state<'all' | 'mine' | null>(null);

  const path = $derived(project.active?.path ?? null);
  const loaded = $derived(routines.listing !== null);
  const views = $derived(routines.views);
  const folderProblem = $derived(routines.listing?.folderProblem ?? null);
  const none = $derived(loaded && folderProblem === null && views.length === 0);
  const counts = $derived(filterCounts(routines.listing?.counts ?? { total: 0, status: {}, when: {}, origin: {} }));
  // "Delete all" removes the local routines and the files that break the format alike. The team's stay.
  const deletable = $derived(views.filter((view) => view.origin === 'local').length);
  const query = $derived(viewState.routinesQuery);
  const when = $derived(viewState.routinesWhen);
  const status = $derived(viewState.routinesStatus);
  // The team's state: null while routines are not shared, and then there is no Source filter, so a kept choice of it counts for nothing.
  const team = $derived(routines.listing?.team ?? null);
  const source = $derived(team ? viewState.routinesSource : 'all');
  const filtered = $derived(when !== 'all' || status !== 'all' || source !== 'all');
  const narrowed = $derived(filtered || query.trim() !== '');
  const shown = $derived(views.filter((view) => inWhen(view, when) && inStatus(view, status) && inSource(view, source) && matchesQuery(view, query)));
  const sort = $derived(viewState.routinesSort);
  const groups = $derived(sortRoutines(groupRoutines(views, shown, narrowed, team !== null), sort));
  const openId = $derived(viewState.routineId);
  const section = $derived(viewState.routinesSection);
  const waiting = $derived(routines.waiting);
  // Routines only run while the app does: with some switched on and the tray setting off, the view says so.
  const scheduled = $derived(views.some((view) => view.status === 'active' || view.status === 'running'));
  const trayOff = $derived(prefs.value?.keepRunningInTray === false);

  const sections = $derived<{ value: RoutinesSection; label: string; icon: IconComponent }[]>([
    { value: 'routines', label: t('routines.section.routines'), icon: CalendarClock },
    { value: 'runs', label: t('routines.section.runs'), icon: History },
  ]);

  const whenOptions = $derived([
    { value: 'all' as RoutineWhenFilter, label: t('routines.filter.all'), count: counts.when.all },
    ...WHEN_KINDS.map((kind) => ({ value: kind as RoutineWhenFilter, label: t(WHEN_LABELS[kind]), icon: WHEN_ICONS[kind], count: counts.when[kind] })),
  ]);

  // Overridden is a state of team routines only: it is listed while some routine is in it.
  const statusOptions = $derived([
    { value: 'all' as RoutineStatusFilter, label: t('routines.filter.all'), count: counts.status.all },
    ...STATUSES.filter((value) => value !== 'duplicate' || counts.status.duplicate > 0 || status === 'duplicate').map((value) => ({ value: value as RoutineStatusFilter, label: t(STATUS_LABELS[value]), count: counts.status[value] })),
  ]);

  const sourceOptions = $derived([
    { value: 'all' as RoutineSource, label: t('routines.filter.all'), count: counts.source.all },
    { value: 'mine' as RoutineSource, label: t('routines.filter.source.mine'), count: counts.source.mine },
    { value: 'team' as RoutineSource, label: t('routines.filter.source.team'), count: counts.source.team },
  ]);

  // A search that finds nothing says so, and so do filters that leave nothing: they stay chosen, with the way to clear them in the row above.
  const emptied = $derived(emptyReason(loaded, shown.length, query, filtered));
  const noMatch = $derived(emptied === null ? null : t(emptied === 'filter' ? 'routines.list.noFilter' : 'routines.list.noMatch'));

  // The header says what waits for the user. How many routines there are is said once, in the row of the filters.
  const context = $derived.by(() => {
    const parts = [routines.pending > 0 ? t('routines.context.pending', { count: routines.pending }) : '', waiting.size > 0 ? t('routines.context.needsYou', { count: waiting.size }) : ''];

    return views.length > 0 ? parts.filter(Boolean).join(' · ') : '';
  });

  // The open routine, kept after its panel closes so it slides out showing the routine it had.
  const live = $derived(routines.find(openId));
  let last = $state.raw<RoutineView | null>(null);
  const shownView = $derived(openId === null ? last : live);
  const running = $derived(shownView ? routines.runOf(shownView.id) : null);

  $effect(() => {
    if (live) {
      last = live;
    }
  });

  // Opening the view, or switching project, reads again; a list already read is refreshed quietly. The team routines are asked for
  // once the list is up, so the view never waits for the network.
  $effect(() => {
    const where = path;

    untrack(() => {
      routines.follow(where);
      void routines.refreshTeam(where);
    });
  });

  // A list that failed to load is read again on the retry schedule, a little later each time, instead of at the poll's pace.
  $effect(() => autoRefresh(() => routines.refresh(), ROUTINES_POLL_MS, { failing: routines.list.status === 'error' }));

  // A panel whose routine was deleted meanwhile, by an agent or elsewhere, closes: there is nothing left to show.
  $effect(() => {
    if (loaded && openId !== null && live === null) {
      viewState.routineId = null;
    }
  });

  // "New routine" from the command palette opens the editor once the view is up.
  $effect(() => {
    if (routines.wantsNew) {
      routines.wantsNew = false;
      create('global');
    }
  });

  async function confirmed(request: ConfirmRequest): Promise<boolean> {
    return (await confirmer?.ask(request)) === true;
  }

  /** Closing the panel puts the focus back on the row it came from. */
  function close(): Promise<void> {
    return closePanel(openId, () => (viewState.routineId = null), (id) => table?.focusRow(id));
  }

  function create(scope: EditorTarget['scope']): void {
    editorTarget = { view: null, scope };
    editorOpen = true;
  }

  function edit(view: RoutineView): void {
    editorTarget = { view, scope: view.routine?.scope ?? 'global' };
    editorOpen = true;
  }

  function saved(view: RoutineView, created: boolean): void {
    const routine = view.routine;

    if (created && routine) {
      toasts.push({ title: t('routines.toast.added', { name: routineTitle(view) }), description: t('routines.panel.status.active', { when: whenText(routine.when) }), tone: 'success' });
    } else if (routine) {
      toasts.push({ title: t('routines.toast.saved', { name: routineTitle(view) }), tone: 'success' });
    }

    viewState.routineId = viewKey(view);
  }

  async function remove(view: RoutineView): Promise<void> {
    const routine = view.routine;
    const accepted = await confirmed({
      title: t('routines.delete.title'),
      description: t('routines.delete.body'),
      subject: { icon: CalendarClock, name: routineTitle(view), details: routine ? `${whenText(routine.when)} · ${routine.scope === 'project' ? t('routines.scope.project') : t('routines.scope.global')}` : view.id },
      confirmLabel: t('routines.delete.confirm'),
    });

    if (accepted && (await routines.remove(view)) && viewState.routineId === viewKey(view)) {
      viewState.routineId = null;
    }
  }

  /** Takes a team routine off the team after a confirm, as it goes for every teammate. */
  async function removeFromTeam(view: RoutineView): Promise<void> {
    const accepted = await confirmed({
      title: t('routines.team.remove.title'),
      description: t('routines.team.remove.body'),
      subject: { icon: UsersRound, name: routineTitle(view), details: view.teamAuthor ? t('routines.field.sharedBy') + ' ' + view.teamAuthor : undefined },
      confirmLabel: t('routines.team.remove.confirm'),
    });

    if (accepted && (await routines.removeTeam(view)) && viewState.routineId === viewKey(view)) {
      viewState.routineId = null;
    }
  }

  /** Copies a team routine to the user's own, and opens the copy. */
  async function copyToMine(view: RoutineView): Promise<void> {
    const copy = await routines.copyTeam(view, path);

    if (copy) {
      viewState.routineId = viewKey(copy);
    }
  }

  /** Shows the runs of a routine in the run history, and a run of it when one is given. */
  function showRuns(view: RoutineView, runId?: string): void {
    viewState.runsRoutine = view.id;
    viewState.runsStatus = 'all';
    viewState.runsPage = 0;
    viewState.runId = runId ?? null;
    viewState.routinesSection = 'runs';
  }

  /** Shows a routine, from the run history, in the list of routines. */
  function showRoutine(routineId: string): void {
    const view = views.find((candidate) => candidate.id === routineId);

    viewState.routineId = view ? viewKey(view) : null;
    viewState.routinesSection = 'routines';
  }

  function handlersOf(view: RoutineView): RoutineHandlers {
    return {
      toggle: (enabled) => void routines.toggle(view, enabled),
      approve: () => void routines.approve(view),
      reject: () => void routines.reject(view),
      run: () => void routines.runNow(view),
      stop: () => {
        const current = routines.runOf(view.id);

        if (current) {
          void routines.stop(current);
        }
      },
      edit: () => edit(view),
      remove: () => void remove(view),
      share: () => void routines.share(view),
      update: () => void routines.updateTeam(view),
      removeTeam: () => void removeFromTeam(view),
      copy: () => void copyToMine(view),
      openChat: () => running && void openRunChat(running),
      openSession: () => running && openRunSession(running),
      showRuns: (runId) => showRuns(view, runId),
      close: () => void close(),
    };
  }
</script>

<div class="flex size-full min-h-0 flex-col overflow-clip">
  <ViewHeader title={t('routines.title')} icon={CalendarClock} {context}>
    {#snippet actions()}
      {#if views.length > 0}
        {#if section === 'routines'}
          <div class="search-width">
            <SearchField bind:value={viewState.routinesQuery} label={t('routines.search.label')} placeholder={t('routines.search.placeholder')} shortcut="mod+f" onkeydown={(event) => emptySearchOnEscape(event, viewState.routinesQuery, () => (viewState.routinesQuery = ''))} />
          </div>
        {/if}

        <FilterMenu label={t('routines.section.label')} options={sections} value={section} onchange={(next) => (viewState.routinesSection = next)} />
      {/if}

      <DocNewButton label={t('routines.new')} onclick={() => create('global')} />
    {/snippet}

    {#snippet fixed()}
      <MoreMenu label={t('routines.menu.label')} panelClass="w-64">
        {#snippet children({ close })}
          {#if team}
            <MenuItem
              tone="danger"
              icon={UserMinus}
              onclick={() => {
                close();
                deleteOpen = 'mine';
              }}>{t('routines.removeMine.action')}</MenuItem
            >
          {/if}
          <MenuItem
            tone="danger"
            icon={Trash2}
            disabled={deletable === 0}
            onclick={() => {
              close();
              deleteOpen = 'all';
            }}>{t('routines.deleteAll.action')}</MenuItem
          >
        {/snippet}
      </MoreMenu>
    {/snippet}
  </ViewHeader>

  <TrayHint show={scheduled && trayOff} onTurnOn={turnOnTray} />

  {#if section === 'runs' && views.length > 0}
    <RunHistory onopenchat={(run) => void openRunChat(run)} onopensession={openRunSession} onroutine={showRoutine} />
  {:else}
    <DocStage>
      {#if routines.list.status === 'error'}
        <div class="grid size-full place-items-center">
          <EmptyState icon={CircleAlert} title={t('routines.error.title')} body={routines.list.error?.message}>
            {#snippet action()}
              <Button variant="primary" onclick={() => void routines.list.refresh()}>{t('routines.tryAgain')}</Button>
            {/snippet}
          </EmptyState>
        </div>
      {:else if !loaded}
        <div class="flex flex-col gap-3 p-8" role="status" aria-label={t('routines.loading')}>
          <Skeleton width="14rem" height="1.75rem" />
          <Skeleton height="2.5rem" />
          <Skeleton height="2.5rem" />
        </div>
      {:else if folderProblem}
        <div class="grid size-full place-items-center">
          <EmptyState icon={CircleAlert} title={t('routines.folderProblem.title')} body={t('routines.folderProblem.body', { reason: folderProblem })}>
            {#snippet action()}
              <Button variant="primary" onclick={() => void routines.list.refresh()}>{t('routines.tryAgain')}</Button>
            {/snippet}
          </EmptyState>
        </div>
      {:else if none}
        <div class="grid size-full place-items-center overflow-y-auto">
          <EmptyState icon={CalendarClock} title={t('routines.empty.title')} body={t('routines.empty.body')}>
            {#snippet action()}
              <div in:rise={{ y: 8, delay: 120 }} class="flex flex-wrap items-center justify-center gap-2">
                <DocNewButton label={t('routines.new')} onclick={() => create('global')} />
              </div>

              <div class="mt-4 w-full max-w-md"><DocMcpHint message={t('routines.mcpHint')} action={t('routines.mcpHint.action')} onsetup={() => onboarding.restart('setup')} /></div>
            {/snippet}
          </EmptyState>
        </div>
      {:else}
        <DocSplit {openId} emptyIcon={CalendarClock} emptyMessage={t('routines.detail.empty')} onclose={() => void close()}>
          {#snippet body()}
            <FilterRow count={tn('routines.count', shown.length)}>
              <DocFilter label={t('routines.list.column.when')} icon={Clock} options={whenOptions} value={when} onchange={(next) => (viewState.routinesWhen = next)} clearLabel={t('routines.filter.clearWhen')} />
              <DocFilter label={t('routines.list.column.status')} options={statusOptions} value={status} onchange={(next) => (viewState.routinesStatus = next)} clearLabel={t('routines.filter.clearStatus')} />
              <!-- Mine and Team only mean something while routines are shared. -->
              {#if team}
                <DocFilter label={t('routines.filter.source')} icon={UsersRound} options={sourceOptions} value={source} onchange={(next) => (viewState.routinesSource = next)} clearLabel={t('routines.filter.clearSource')} />
              {/if}
            </FilterRow>

            {#if noMatch}
              <p class="min-h-0 flex-1 px-4 py-10 text-center text-sm text-fg-3" role="status">{noMatch}</p>
            {:else}
              <RoutineList
                bind:table
                {groups}
                {sort}
                onsort={(next) => (viewState.routinesSort = next)}
                {openId}
                projectName={project.active ? folderName(project.active.path) : null}
                teamProblem={team?.problem ?? null}
                teamTrusted={team?.trust ?? false}
                busy={routines.busy}
                {waiting}
                onopen={(id) => (viewState.routineId = id)}
                onmove={(id) => openId !== null && (viewState.routineId = id)}
                ontoggle={(view, enabled) => void routines.toggle(view, enabled)}
                onadd={create}
              />
            {/if}
          {/snippet}

          {#snippet panel(inline)}
            <RoutinePanel
              open={openId !== null}
              {inline}
              view={shownView}
              {running}
              busy={shownView !== null && routines.busy.has(viewKey(shownView))}
              sharing={team !== null}
              canUpdate={shownView !== null && originalOf(views, shownView) !== null}
              projectOpen={project.active !== null}
              handlers={shownView ? handlersOf(shownView) : NO_HANDLERS}
            />
          {/snippet}
        </DocSplit>
      {/if}
    </DocStage>
  {/if}
</div>

<RoutineEditor bind:open={editorOpen} target={editorTarget} onsaved={saved} />
<DeleteRoutinesDialog bind:open={() => deleteOpen !== null, (next) => !next && (deleteOpen = null)} kind={deleteOpen ?? 'all'} count={deletable} onremoved={() => (viewState.routineId = null)} />
<SessionConfirm bind:this={confirmer} />
