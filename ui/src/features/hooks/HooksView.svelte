<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import Clock from '@lucide/svelte/icons/clock';
  import Plus from '@lucide/svelte/icons/plus';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import Users from '@lucide/svelte/icons/users';
  import Webhook from '@lucide/svelte/icons/webhook';
  import { tick, untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import DetailEmpty from '$lib/components/DetailEmpty.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import { ICON } from '$lib/components/icon';
  import FilterMenu from '$lib/components/FilterMenu.svelte';
  import FilterRow from '$lib/components/FilterRow.svelte';
  import MenuItem from '$lib/components/MenuItem.svelte';
  import MoreMenu from '$lib/components/MoreMenu.svelte';
  import SearchField from '$lib/components/SearchField.svelte';
  import { SIDE_PANEL_WIDTH } from '$lib/components/side-panel';
  import Skeleton from '$lib/components/Skeleton.svelte';
    import ViewHeader from '$lib/components/ViewHeader.svelte';
  import { rise } from '$lib/design/motion';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { HookScope } from '$lib/ipc/bindings/HookScope';
  import type { HookView } from '$lib/ipc/bindings/HookView';
  import { project } from '$lib/stores/project.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';
  import { type HookStatusFilter, type HookWhen, viewState } from '$lib/stores/view-state.svelte';
  import { autoRefresh } from '$lib/utils/auto-refresh';
  import { escapeCloses } from '$lib/utils/escape-closes';

  import SessionConfirm, { type ConfirmRequest } from '../sessions/SessionConfirm.svelte';
  import DesktopMcpHint from './DesktopMcpHint.svelte';
  import DeleteAllDialog from './DeleteAllDialog.svelte';
  import FamilyDialog from './FamilyDialog.svelte';
  import { EVENT_LABELS, EVENT_WHEN, EVENTS, filterCounts, folderName, groupHooks, hookTitle, inStatus, inWhen, matchesQuery, STATUS_LABELS, STATUSES, type WaitingFor, waitingFor } from './hook-format';
  import { type EditorTarget } from './hook-form';
  import HookEditor from './HookEditor.svelte';
  import HookList from './HookList.svelte';
  import HookPanel from './HookPanel.svelte';
  import { hooks } from './hooks-data.svelte';
  import PausedNotice from './PausedNotice.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';

  /** The width of the view from which it has room for two columns: the list, and the detail, which is always there. Below it the detail slides over the list. */
  const WIDE_FROM_PX = 1360;

  /** A backstop for the change event of the hooks folder: an open view looks again now and then. */
  const REFRESH_MS = 15_000;

  let width = $state(0);
  let confirmer: SessionConfirm | undefined = $state();
  let list: HookList | undefined = $state();
  let editorOpen = $state(false);
  let editorTarget = $state<EditorTarget>({ view: null, scope: 'global' });
  let familyOpen = $state(false);
  let deleteAllOpen = $state(false);

  const wide = $derived(width >= WIDE_FROM_PX);
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
  const filtered = $derived(when !== 'all' || status !== 'all');
  const narrowed = $derived(filtered || query.trim() !== '');
  const shown = $derived(views.filter((view) => inWhen(view, when) && inStatus(view, status) && matchesQuery(view, query)));
  const groups = $derived(groupHooks(views, shown, narrowed));
  const openId = $derived(viewState.hookId);

  const whenOptions = $derived([
    { value: 'all' as HookWhen, label: t('hooks.filter.all'), count: counts.when.all },
    ...EVENTS.map((event) => ({ value: event as HookWhen, label: t(EVENT_LABELS[event]), count: counts.when[event] })),
  ]);

  const statusOptions = $derived([
    { value: 'all' as HookStatusFilter, label: t('hooks.filter.all'), count: counts.status.all },
    ...STATUSES.map((value) => ({ value: value as HookStatusFilter, label: t(STATUS_LABELS[value]), count: counts.status[value] })),
  ]);

  // A search that finds nothing says so, and so do filters that leave nothing: they stay chosen, with the way to clear them in the row above.
  const noMatch = $derived.by(() => {
    if (!loaded || shown.length > 0) {
      return null;
    }

    return query.trim() === '' && filtered ? t('hooks.list.noFilter') : t('hooks.list.noMatch');
  });

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

  // Opening the view, or switching project, reads again; a list already read is refreshed quietly.
  $effect(() => {
    const where = path;

    untrack(() => hooks.follow(where));
  });

  $effect(() => autoRefresh(() => void hooks.refresh(), REFRESH_MS));

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
  async function close(): Promise<void> {
    const id = openId;

    viewState.hookId = null;

    if (id !== null) {
      await tick();
      list?.focusRow(id);
    }
  }

  function onKeydown(event: KeyboardEvent): void {
    escapeCloses(event, openId !== null, () => void close());
  }

  /** Escape in a filled search empties it, and only then closes the panel. */
  function clearSearchOnEscape(event: KeyboardEvent): void {
    if (event.key === 'Escape' && viewState.hooksQuery !== '') {
      event.preventDefault();
      event.stopPropagation();
      viewState.hooksQuery = '';
    }
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
      toasts.push({ title: t('hooks.toast.added', { name: hookTitle(view), when: t(EVENT_WHEN[hook.event]) }), tone: 'success' });
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

    if (confirmed && (await hooks.remove(view)) && viewState.hookId === view.id) {
      viewState.hookId = null;
    }
  }
</script>

<div class="flex size-full min-h-0 flex-col overflow-clip">
  <ViewHeader title={t('hooks.title')} icon={Webhook} {context}>
    {#snippet actions()}
      {#if views.length > 0}
        <div class="w-[340px] max-[900px]:w-[220px]">
          <SearchField bind:value={viewState.hooksQuery} label={t('hooks.search.label')} placeholder={t('hooks.search.placeholder')} shortcut="mod+f" onkeydown={clearSearchOnEscape} />
        </div>
      {/if}

      <Button variant="primary" onclick={() => create('global')}>
        {#snippet leading()}<Plus size={ICON.sm} aria-hidden="true" />{/snippet}
        {t('hooks.new')}
      </Button>

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

  <!-- Everything that is bigger than the view is clipped here, not just hidden: a clipped box cannot be scrolled at all, so
       nothing in the view (a focused row, a panel on its way in) can push the shell around it up or down. -->
  <div bind:clientWidth={width} class="relative min-h-0 flex-1 overflow-clip">
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
              <Button variant="primary" onclick={() => create('global')}>
                {#snippet leading()}<Plus size={ICON.sm} aria-hidden="true" />{/snippet}
                {t('hooks.new')}
              </Button>
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
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="absolute inset-0 flex overflow-clip" onkeydown={onKeydown}>
        <!-- Below the wide layout the panel slides over the list: the filters and the list keep their width. -->
        <div class="flex min-h-0 min-w-0 flex-1 flex-col">
          <FilterRow count={tn('hooks.count', shown.length)}>
            <FilterMenu
              label={t('hooks.list.column.when')}
              prefix={t('hooks.list.column.when')}
              icon={Clock}
              options={whenOptions}
              value={when}
              onchange={(next) => (viewState.hooksWhen = next)}
              onclear={() => (viewState.hooksWhen = 'all')}
              clearLabel={t('hooks.filter.clearWhen')}
              align="start"
            />
            <FilterMenu
              label={t('hooks.list.column.status')}
              prefix={t('hooks.list.column.status')}
              options={statusOptions}
              value={status}
              onchange={(next) => (viewState.hooksStatus = next)}
              onclear={() => (viewState.hooksStatus = 'all')}
              clearLabel={t('hooks.filter.clearStatus')}
              align="start"
            />
          </FilterRow>

          {#if noMatch}
            <p class="min-h-0 flex-1 px-4 py-10 text-center text-sm text-fg-3" role="status">{noMatch}</p>
          {:else}
            <HookList
              bind:this={list}
              {groups}
              {openId}
              projectName={project.active ? folderName(project.active.path) : null}
              busy={hooks.busy}
              onopen={(id) => (viewState.hookId = id)}
              onmove={(id) => openId !== null && (viewState.hookId = id)}
              ontoggle={(view, enabled) => void hooks.toggle(view, enabled)}
              onadd={create}
            />
          {/if}
        </div>

        {#snippet panel(inline: boolean)}
          <HookPanel
            open={openId !== null}
            {inline}
            view={shownView}
            busy={shownView !== null && hooks.busy.has(shownView.id)}
            ontoggle={(enabled) => shownView && void hooks.toggle(shownView, enabled)}
            onapprove={() => shownView && void hooks.approve(shownView)}
            onreject={() => shownView && void hooks.reject(shownView)}
            onedit={() => shownView && edit(shownView)}
            ondelete={() => shownView && void remove(shownView)}
            onclose={() => void close()}
          />
        {/snippet}

        {#if wide}
          <!-- The detail has its own column, always there: what is chosen swaps in with a short fade, with nothing sliding. -->
          <div class="relative flex shrink-0 flex-col border-l border-line-strong bg-surface-1" style:width={SIDE_PANEL_WIDTH}>
            {#if openId === null}
              <DetailEmpty icon={Webhook} message={t('hooks.detail.empty')} />
            {:else}
              {#key openId}
                <div in:rise={{ y: 6, duration: 240 }} class="flex min-h-0 flex-1 flex-col">
                  {@render panel(true)}
                </div>
              {/key}
            {/if}
          </div>
        {:else}
          {@render panel(false)}
        {/if}
      </div>
    {/if}
  </div>
</div>

<HookEditor bind:open={editorOpen} target={editorTarget} onsaved={saved} />
<FamilyDialog bind:open={familyOpen} />
<DeleteAllDialog bind:open={deleteAllOpen} count={deletable} onremoved={() => (viewState.hookId = null)} />
<SessionConfirm bind:this={confirmer} />
