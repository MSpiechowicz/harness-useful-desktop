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
  import type { DocTableHandle } from '$lib/components/docs/doc-groups';
  import { closePanel, emptyReason, emptySearchOnEscape } from '$lib/components/docs/doc-view';
  import DocSplit from '$lib/components/docs/DocSplit.svelte';
  import DocStage from '$lib/components/docs/DocStage.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import { ICON } from '$lib/components/icon';
  import FilterRow from '$lib/components/FilterRow.svelte';
  import MenuItem from '$lib/components/MenuItem.svelte';
  import MoreMenu from '$lib/components/MoreMenu.svelte';
  import SearchField from '$lib/components/SearchField.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import { rise } from '$lib/design/motion';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { HookScope } from '$lib/ipc/bindings/HookScope';
  import type { HookView } from '$lib/ipc/bindings/HookView';
  import { HOOKS_POLL_MS } from '$lib/stores/live';
  import { project } from '$lib/stores/project.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';
  import { type HookSource, type HookStatusFilter, type HookWhen, viewState } from '$lib/stores/view-state.svelte';
  import { autoRefresh } from '$lib/utils/auto-refresh';

  import SessionConfirm, { type ConfirmRequest } from '../sessions/SessionConfirm.svelte';
  import DesktopMcpHint from './DesktopMcpHint.svelte';
  import DeleteAllDialog from './DeleteAllDialog.svelte';
  import FamilyDialog from './FamilyDialog.svelte';
  import { EVENT_LABELS, EVENT_WHEN, EVENTS, filterCounts, folderName, groupHooks, hookTitle, inSource, inStatus, inWhen, matchesQuery, sortHooks, STATUS_LABELS, STATUSES, type WaitingFor, viewKey, waitingFor } from './hook-format';
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
  let editorTarget = $state<EditorTarget>({ view: null, scope: 'global' });
  let familyOpen = $state(false);
  let deleteAllOpen = $state(false);

  const path = $derived(project.active?.path ?? null);
  const loaded = $derived(hooks.listing !== null);
  const views = $derived(hooks.views);
  const folderProblem = $derived(hooks.listing?.folderProblem ?? null);
  const none = $derived(loaded && folderProblem === null && views.length === 0);
  const counts = $derived(filterCounts(views));
  // "Delete all" removes the hooks and the files that break the format alike.
  const deletable = $derived(views.length);
  const query = $derived(viewState.hooksQuery);
  const when = $derived(viewState.hooksWhen);
  const status = $derived(viewState.hooksStatus);
  // The team's state: null while hooks are not shared, and then there is no Source filter, so a kept choice of it counts for nothing.
  const team = $derived(hooks.listing?.team ?? null);
  const source = $derived(team ? viewState.hooksSource : 'all');
  const filtered = $derived(when !== 'all' || status !== 'all' || source !== 'all');
  const narrowed = $derived(filtered || query.trim() !== '');
  const shown = $derived(views.filter((view) => inWhen(view, when) && inStatus(view, status) && inSource(view, source) && matchesQuery(view, query)));
  const sort = $derived(viewState.hooksSort);
  const groups = $derived(sortHooks(groupHooks(views, shown, narrowed, team !== null), sort));
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

  $effect(() => {
    if (live) {
      last = live;
    }
  });

  // Opening the view, or switching project, reads again; a list already read is refreshed quietly. The team hooks are asked for
  // once the list is up, so the view never waits for the network.
  $effect(() => {
    const where = path;

    untrack(() => {
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

  function create(scope: HookScope): void {
    editorTarget = { view: null, scope };
    editorOpen = true;
  }

  function edit(view: HookView): void {
    editorTarget = { view, scope: view.hook?.scope ?? 'global' };
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
      subject: { icon: Webhook, name: hookTitle(view), details: hook ? `${t(EVENT_LABELS[hook.event])} · ${hook.scope === 'project' ? t('hooks.scope.project') : t('hooks.scope.global')}` : view.id },
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

  /** Copies a team hook to the user's own, and opens the copy. */
  async function copyToMine(view: HookView): Promise<void> {
    const copy = await hooks.copyTeam(view, path);

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

      <DocNewButton label={t('hooks.new')} onclick={() => create('global')} />
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
            disabled={deletable === 0}
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
              <DocNewButton label={t('hooks.new')} onclick={() => create('global')} />
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
            <DocFilter label={t('hooks.list.column.when')} icon={Clock} options={whenOptions} value={when} onchange={(next) => (viewState.hooksWhen = next)} clearLabel={t('hooks.filter.clearWhen')} />
            <DocFilter label={t('hooks.list.column.status')} options={statusOptions} value={status} onchange={(next) => (viewState.hooksStatus = next)} clearLabel={t('hooks.filter.clearStatus')} />
            <!-- Mine and Team only mean something while hooks are shared. -->
            {#if team}
              <DocFilter label={t('hooks.filter.source')} icon={UsersRound} options={sourceOptions} value={source} onchange={(next) => (viewState.hooksSource = next)} clearLabel={t('hooks.filter.clearSource')} />
            {/if}
          </FilterRow>

          {#if noMatch}
            <p class="min-h-0 flex-1 px-4 py-10 text-center text-sm text-fg-3" role="status">{noMatch}</p>
          {:else}
            <HookList
              bind:table
              {groups}
              {sort}
              onsort={(next) => (viewState.hooksSort = next)}
              {openId}
              projectName={project.active ? folderName(project.active.path) : null}
              teamProblem={team?.problem ?? null}
              teamTrusted={team?.trust ?? false}
              busy={hooks.busy}
              onopen={(id) => (viewState.hookId = id)}
              onmove={(id) => openId !== null && (viewState.hookId = id)}
              ontoggle={(view, enabled) => void hooks.toggle(view, enabled)}
              onadd={create}
            />
          {/if}
        {/snippet}

        {#snippet panel(inline)}
          <HookPanel
            open={openId !== null}
            {inline}
            view={shownView}
            busy={shownView !== null && hooks.busy.has(viewKey(shownView))}
            sharing={team !== null}
            canUpdate={shownView !== null && originalOf(views, shownView) !== null}
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
<DeleteAllDialog bind:open={deleteAllOpen} count={deletable} onremoved={() => (viewState.hookId = null)} />
<SessionConfirm bind:this={confirmer} />
