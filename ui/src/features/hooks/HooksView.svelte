<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import Clock from '@lucide/svelte/icons/clock';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import Users from '@lucide/svelte/icons/users';
  import UsersRound from '@lucide/svelte/icons/users-round';
  import Webhook from '@lucide/svelte/icons/webhook';
  import { untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import DocFilter from '$lib/components/docs/DocFilter.svelte';
  import DocNewButton from '$lib/components/docs/DocNewButton.svelte';
  import { DOC_PAGE_SIZE, type DocTableHandle } from '$lib/components/docs/doc-groups';
  import { revealOpenRow, startPageOver } from '$lib/components/docs/doc-page.svelte';
  import { deleteReach, type DocScope, docScope, docScopeOptions, inScope, projectOf, resolveScope, scopeSortValue, scopeText } from '$lib/components/docs/doc-scope';
  import { docTarget, newDocTarget } from '$lib/components/docs/doc-target';
  import { closePanel, emptyReason, emptySearchOnEscape } from '$lib/components/docs/doc-view';
  import DocSplit from '$lib/components/docs/DocSplit.svelte';
  import DocStage from '$lib/components/docs/DocStage.svelte';
  import DocTeamNotice from '$lib/components/docs/DocTeamNotice.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import { ICON } from '$lib/components/icon';
  import FilterRow from '$lib/components/FilterRow.svelte';
  import MenuItem from '$lib/components/MenuItem.svelte';
  import MoreMenu from '$lib/components/MoreMenu.svelte';
  import ScopeFilter from '$lib/components/scope/ScopeFilter.svelte';
  import type { ScopeChoice } from '$lib/components/scope/scope-options';
  import SearchField from '$lib/components/SearchField.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import { rise } from '$lib/design/motion';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { HookView } from '$lib/ipc/bindings/HookView';
  import { HOOKS_POLL_MS } from '$lib/stores/live';
  import { project } from '$lib/stores/project.svelte';
  import { projectScopes, revalidateProjectScopes } from '$lib/stores/project-scopes.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';
  import { type HookSource, type HookStatusFilter, type HookWhen, viewState } from '$lib/stores/view-state.svelte';
  import { autoRefresh } from '$lib/utils/auto-refresh';
  import { clampPage, pageOf } from '$lib/utils/pagination';

  import SessionConfirm, { type ConfirmRequest } from '../sessions/SessionConfirm.svelte';
  import DesktopMcpHint from './DesktopMcpHint.svelte';
  import DeleteAllDialog from './DeleteAllDialog.svelte';
  import FamilyDialog from './FamilyDialog.svelte';
  import { deletableCount, EVENT_LABELS, EVENT_WHEN, EVENTS, filterCounts, hookDoc, hookTitle, inSource, inStatus, inWhen, matchesQuery, sortHooks, STATUS_LABELS, STATUSES, type WaitingFor, viewKey, waitingFor } from './hook-format';
  import { type EditorTarget } from './hook-form';
  import { originalOf } from './hook-team';
  import HookEditor from './HookEditor.svelte';
  import HookList from './HookList.svelte';
  import HookPanel from './HookPanel.svelte';
  import { hooks } from './hooks-data.svelte';
  import PausedNotice from './PausedNotice.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';

  let confirmer: SessionConfirm | undefined = $state();
  let table: DocTableHandle | undefined = $state();
  let editorOpen = $state(false);
  let editorTarget = $state<EditorTarget>({ view: null, target: { kind: 'global' } });
  let familyOpen = $state(false);
  let deleteAllOpen = $state(false);

  const path = $derived(project.active?.path ?? null);
  const loaded = $derived(hooks.listing !== null);
  const views = $derived(hooks.views);
  const folderProblem = $derived(hooks.listing?.folderProblem ?? null);
  const none = $derived(loaded && folderProblem === null && views.length === 0);
  const counts = $derived(filterCounts(views));
  const projects = $derived(projectScopes.data ?? []);
  // "Delete all" removes the global hooks, the open project's and the files that break the format alike. Another project's hooks and the team's stay.
  const reach = $derived(deleteReach(project.active, projects));
  const deletable = $derived(deletableCount(views, reach.root));
  const query = $derived(viewState.hooksQuery);
  const when = $derived(viewState.hooksWhen);
  const status = $derived(viewState.hooksStatus);
  // The team's state: null while hooks are not shared, and then there is no Source filter, so a kept choice of it counts for nothing.
  const team = $derived(hooks.listing?.team ?? null);
  const source = $derived(team ? viewState.hooksSource : 'all');
  // The Scope filter lists the projects on this computer and the repositories only the team's hooks name. A choice that is gone from it (a project forgotten) counts as All once they are read.
  const scopeOptions = $derived(docScopeOptions(views.map(hookDoc), projects, { all: t('components.scope.all'), global: t('components.scope.global'), remoteOnly: t('components.scope.remoteOnly') }));
  const scope = $derived<ScopeChoice>(resolveScope(viewState.hooksScope as ScopeChoice, scopeOptions, loaded && projectScopes.data !== null));
  const filtered = $derived(scope !== 'all' || when !== 'all' || status !== 'all' || source !== 'all');
  const shown = $derived(inScope(views, scope, hookDoc, projects).filter((view) => inWhen(view, when) && inStatus(view, status) && inSource(view, source) && matchesQuery(view, query)));
  const sort = $derived(viewState.hooksSort);
  const sorted = $derived(sortHooks(shown, sort, (view) => scopeSortValue(scopeOf(view))));
  const pagination = $derived({ page: clampPage(viewState.hooksPage, sorted.length, DOC_PAGE_SIZE), pageSize: DOC_PAGE_SIZE, total: sorted.length, onpage: (page: number) => (viewState.hooksPage = page) });
  const openId = $derived(viewState.hookId);

  const whenOptions = $derived([
    { value: 'all' as HookWhen, label: t('hooks.filter.all'), count: counts.when.all },
    ...EVENTS.map((event) => ({ value: event as HookWhen, label: t(EVENT_LABELS[event]), count: counts.when[event] })),
  ]);

  // Overridden is a state of team hooks only: it is listed while some hook is in it.
  const statusOptions = $derived([
    { value: 'all' as HookStatusFilter, label: t('hooks.filter.all'), count: counts.status.all },
    ...STATUSES.filter((value) => value !== 'duplicate' || counts.status.duplicate > 0 || status === 'duplicate').map((value) => ({ value: value as HookStatusFilter, label: t(STATUS_LABELS[value]), count: counts.status[value] })),
  ]);

  const sourceOptions = $derived([
    { value: 'all' as HookSource, label: t('hooks.filter.all'), count: counts.source.all },
    { value: 'mine' as HookSource, label: t('hooks.filter.source.mine'), count: counts.source.mine },
    { value: 'team' as HookSource, label: t('hooks.filter.source.team'), count: counts.source.team },
  ]);

  // A search that finds nothing says so, and so do filters that leave nothing: they stay chosen, with the way to clear them in the row above.
  const emptied = $derived(emptyReason(loaded, shown.length, query, filtered));
  const noMatch = $derived(emptied === null ? null : t(emptied === 'filter' ? 'hooks.list.noFilter' : 'hooks.list.noMatch'));

  // The header says what waits for the user. How many hooks there are is said once, in the row of the filters.
  const context = $derived(hooks.pending > 0 && views.length > 0 ? t('hooks.context.pending', { count: hooks.pending }) : '');

  // Paused hooks wait for a Harness family member: the notice says which, once for each.
  const waiting = $derived.by(() => {
    const members = new Map<string, WaitingFor & { count: number }>();

    for (const view of views) {
      const needs = view.status === 'paused' && view.hook ? waitingFor(view.hook, prefs.value?.memoryEnabled !== false) : null;

      if (needs) {
        const key = `${needs.member}:${needs.off}`;

        members.set(key, { ...needs, count: (members.get(key)?.count ?? 0) + 1 });
      }
    }

    return [...members.values()];
  });

  // The open hook, kept after its panel closes so it slides out showing the hook it had.
  const live = $derived(hooks.find(openId));
  let last = $state.raw<HookView | null>(null);
  const shownView = $derived(openId === null ? last : live);
  // A team hook of a repository that is not on this computer has no folder to be copied to.
  const copyLock = $derived(shownView?.origin === 'team' && copyFolder(shownView) === undefined ? t('hooks.team.copy.noProject') : null);

  $effect(() => {
    if (live) {
      last = live;
    }
  });

  // Whatever narrows or reorders the table starts it from its first page again.
  startPageOver(() => JSON.stringify([query, when, status, source, scope, sort]), () => (viewState.hooksPage = 0));

  // A hook that opens (one just saved) shows in the table, whatever the filters and the page were.
  revealOpenRow({
    openId: () => openId,
    listed: (id) => hooks.find(id) !== null,
    indexOf: (id) => {
      const index = sorted.findIndex((view) => viewKey(view) === id);

      return index < 0 ? null : index;
    },
    pageSize: DOC_PAGE_SIZE,
    setPage: (page) => (viewState.hooksPage = page),
    clearFilters: () => {
      viewState.hooksQuery = '';
      viewState.hooksScope = 'all';
      viewState.hooksWhen = 'all';
      viewState.hooksStatus = 'all';
      viewState.hooksSource = 'all';
    },
  });

  // Opening the view, or switching project, reads again; a list already read is refreshed quietly. The team hooks are asked for
  // once the list is up, so the view never waits for the network.
  $effect(() => {
    const where = path;

    untrack(() => {
      revalidateProjectScopes();
      hooks.follow(where);
      void hooks.refreshTeam(where);
    });
  });

  // A list that failed to load is read again on the retry schedule, a little later each time, instead of at the poll's pace.
  $effect(() => autoRefresh(() => hooks.refresh(), HOOKS_POLL_MS, { failing: hooks.list.status === 'error' }));

  // A hook an agent (or the user, elsewhere) deleted while its panel was open is no longer there to show.
  $effect(() => {
    if (loaded && openId !== null && live === null) {
      viewState.hookId = null;
    }
  });

  function ask(request: ConfirmRequest): Promise<boolean> {
    return confirmer?.ask(request) ?? Promise.resolve(false);
  }

  /** Closes the panel and hands the focus back to the row it was opened from. */
  function close(): Promise<void> {
    return closePanel(openId, () => (viewState.hookId = null), (id) => table?.focusRow(id));
  }

  /** What a hook's Scope cell says. */
  function scopeOf(view: HookView): DocScope {
    return docScope(hookDoc(view), projects);
  }

  /** A new hook starts out for the project the Scope filter shows, else for every project. */
  function create(): void {
    editorTarget = { view: null, target: newDocTarget(scope, projects) };
    editorOpen = true;
  }

  function edit(view: HookView): void {
    editorTarget = { view, target: docTarget(hookDoc(view), projects) };
    editorOpen = true;
  }

  function saved(view: HookView, created: boolean): void {
    const hook = view.hook;

    if (created && hook) {
      toasts.push({ title: t('hooks.toast.added', { name: hookTitle(view) }), description: t('hooks.toast.runs', { when: t(EVENT_WHEN[hook.event]) }), tone: 'success' });
    } else if (hook) {
      toasts.push({ title: t('hooks.toast.saved', { name: hookTitle(view) }), tone: 'success' });
    }

    viewState.hookId = view.id;
  }

  async function remove(view: HookView): Promise<void> {
    const hook = view.hook;
    const confirmed = await ask({
      title: t('hooks.delete.title'),
      description: t('hooks.delete.body'),
      subject: { icon: Webhook, name: hookTitle(view), details: hook ? `${t(EVENT_LABELS[hook.event])} · ${scopeText(scopeOf(view))}` : view.id },
      confirmLabel: t('hooks.delete.confirm'),
    });

    if (confirmed && (await hooks.remove(view)) && viewState.hookId === viewKey(view)) {
      viewState.hookId = null;
    }
  }

  /** Takes a team hook off the team after a confirm, as it goes for every teammate. */
  async function removeFromTeam(view: HookView): Promise<void> {
    const confirmed = await ask({
      title: t('hooks.team.remove.title'),
      description: t('hooks.team.remove.body'),
      subject: { icon: Users, name: hookTitle(view), details: view.teamAuthor ? t('hooks.field.sharedBy') + ' ' + view.teamAuthor : undefined },
      confirmLabel: t('hooks.team.remove.confirm'),
    });

    if (confirmed && (await hooks.removeTeam(view)) && viewState.hookId === viewKey(view)) {
      viewState.hookId = null;
    }
  }

  /** The folder a team hook is copied for: its repository's project on this computer, the open project for a global hook. Undefined for a repository that is not on this computer. */
  function copyFolder(view: HookView): string | null | undefined {
    return view.hook?.scope === 'project' ? projectOf(hookDoc(view), projects)?.path : path;
  }

  /** Copies a team hook to the user's own, and opens the copy. */
  async function copyToMine(view: HookView): Promise<void> {
    const folder = copyFolder(view);

    if (folder === undefined) {
      return;
    }

    const copy = await hooks.copyTeam(view, folder);

    if (copy) {
      viewState.hookId = viewKey(copy);
    }
  }
</script>

<div class="flex size-full min-h-0 flex-col overflow-clip">
  <ViewHeader title={t('hooks.title')} icon={Webhook} {context}>
    {#snippet actions()}
      {#if views.length > 0}
        <div class="search-width">
          <SearchField bind:value={viewState.hooksQuery} label={t('hooks.search.label')} placeholder={t('hooks.search.placeholder')} shortcut="mod+f" onkeydown={(event) => emptySearchOnEscape(event, viewState.hooksQuery, () => (viewState.hooksQuery = ''))} />
        </div>
      {/if}

      <DocNewButton label={t('hooks.new')} onclick={create} />
    {/snippet}

    {#snippet fixed()}
      <MoreMenu label={t('hooks.menu.label')} panelClass="w-60">
        {#snippet children({ close })}
          <MenuItem
            icon={Users}
            onclick={() => {
              close();
              familyOpen = true;
            }}>{t('hooks.menu.family')}</MenuItem
          >
          <MenuItem
            tone="danger"
            icon={Trash2}
            disabled={deletable === 0 || !reach.known}
            onclick={() => {
              close();
              deleteAllOpen = true;
            }}>{t('hooks.deleteAll.action')}</MenuItem
          >
        {/snippet}
      </MoreMenu>
    {/snippet}
  </ViewHeader>

  {#each waiting as notice (`${notice.member}:${notice.off}`)}
    <PausedNotice member={notice.member} off={notice.off} count={notice.count} />
  {/each}

  <DocStage>
    {#if hooks.list.status === 'error'}
      <div class="grid size-full place-items-center">
        <EmptyState icon={CircleAlert} title={t(hooks.list.error?.code === 'unsupported' ? 'hooks.unsupported.title' : 'hooks.error.title')} body={hooks.list.error?.message}>
          {#snippet action()}
            <Button variant="primary" onclick={() => void hooks.list.refresh()}>{t('hooks.tryAgain')}</Button>
          {/snippet}
        </EmptyState>
      </div>
    {:else if !loaded}
      <div class="flex flex-col gap-3 p-8" role="status" aria-label={t('hooks.loading')}>
        <Skeleton width="14rem" height="1.75rem" />
        <Skeleton height="2.5rem" />
        <Skeleton height="2.5rem" />
      </div>
    {:else if folderProblem}
      <div class="grid size-full place-items-center">
        <EmptyState icon={CircleAlert} title={t('hooks.folderProblem.title')} body={t('hooks.folderProblem.body', { reason: folderProblem })}>
          {#snippet action()}
            <Button variant="primary" onclick={() => void hooks.list.refresh()}>{t('hooks.tryAgain')}</Button>
          {/snippet}
        </EmptyState>
      </div>
    {:else if none}
      <div class="grid size-full place-items-center overflow-y-auto">
        <EmptyState icon={Webhook} title={t('hooks.empty.title')} body={t('hooks.empty.body')}>
          {#snippet action()}
            <div in:rise={{ y: 8, delay: 120 }} class="flex flex-wrap items-center justify-center gap-2">
              <DocNewButton label={t('hooks.new')} onclick={create} />
              <Button onclick={() => (familyOpen = true)}>
                {#snippet leading()}<Users size={ICON.sm} aria-hidden="true" />{/snippet}
                {t('hooks.menu.family')}
              </Button>
            </div>

            <div class="mt-4 w-full max-w-md"><DesktopMcpHint /></div>
          {/snippet}
        </EmptyState>
      </div>
    {:else}
      <DocSplit {openId} emptyIcon={Webhook} emptyMessage={t('hooks.detail.empty')} onclose={() => void close()}>
        {#snippet body()}
          <FilterRow count={tn('hooks.count', shown.length)}>
            <ScopeFilter options={scopeOptions} value={scope} onchange={(next) => (viewState.hooksScope = next)} />
            <DocFilter label={t('hooks.list.column.when')} icon={Clock} options={whenOptions} value={when} onchange={(next) => (viewState.hooksWhen = next)} clearLabel={t('hooks.filter.clearWhen')} />
            <DocFilter label={t('hooks.list.column.status')} options={statusOptions} value={status} onchange={(next) => (viewState.hooksStatus = next)} clearLabel={t('hooks.filter.clearStatus')} />
            <!-- Mine and Team only mean something while hooks are shared. -->
            {#if team}
              <DocFilter label={t('hooks.filter.source')} icon={UsersRound} options={sourceOptions} value={source} onchange={(next) => (viewState.hooksSource = next)} clearLabel={t('hooks.filter.clearSource')} />
            {/if}
          </FilterRow>

          <DocTeamNotice problem={team?.problem ? t('hooks.list.teamProblem', { reason: team.problem }) : null} trusted={team?.trust ?? false} trustedLabel={t('hooks.list.teamTrusted')} />

          {#if noMatch}
            <p class="min-h-0 flex-1 px-4 py-10 text-center text-sm text-fg-3" role="status">{noMatch}</p>
          {:else}
            <HookList
              bind:table
              rows={pageOf(sorted, viewState.hooksPage, DOC_PAGE_SIZE)}
              {pagination}
              {sort}
              onsort={(next) => (viewState.hooksSort = next)}
              {openId}
              {scopeOf}
              busy={hooks.busy}
              onopen={(id) => (viewState.hookId = id)}
              onmove={(id) => openId !== null && (viewState.hookId = id)}
              ontoggle={(view, enabled) => void hooks.toggle(view, enabled)}
            />
          {/if}
        {/snippet}

        {#snippet panel(inline)}
          <HookPanel
            open={openId !== null}
            {inline}
            view={shownView}
            {projects}
            busy={shownView !== null && hooks.busy.has(viewKey(shownView))}
            sharing={team !== null}
            canUpdate={shownView !== null && originalOf(views, shownView) !== null}
            {copyLock}
            ontoggle={(enabled) => shownView && void hooks.toggle(shownView, enabled)}
            onapprove={() => shownView && void hooks.approve(shownView)}
            onreject={() => shownView && void hooks.reject(shownView)}
            onedit={() => shownView && edit(shownView)}
            ondelete={() => shownView && void remove(shownView)}
            onshare={() => shownView && void hooks.share(shownView)}
            onupdate={() => shownView && void hooks.updateTeam(shownView)}
            onremove={() => shownView && void removeFromTeam(shownView)}
            oncopy={() => shownView && void copyToMine(shownView)}
            onclose={() => void close()}
          />
        {/snippet}
      </DocSplit>
    {/if}
  </DocStage>
</div>

<HookEditor bind:open={editorOpen} target={editorTarget} onsaved={saved} />
<FamilyDialog bind:open={familyOpen} />
<DeleteAllDialog bind:open={deleteAllOpen} count={deletable} {reach} onremoved={() => (viewState.hookId = null)} />
<SessionConfirm bind:this={confirmer} />
