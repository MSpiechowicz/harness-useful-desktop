<script lang="ts">
  import FileCode from '@lucide/svelte/icons/file-code';
  import ShieldCheck from '@lucide/svelte/icons/shield-check';
  import { untrack } from 'svelte';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import FilterMenu from '$lib/components/FilterMenu.svelte';
  import { ICON } from '$lib/components/icon';
  import type { Column } from '$lib/components/table';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { ScriptDetail } from '$lib/ipc/bindings/ScriptDetail';
  import type { ScriptGroup } from '$lib/ipc/bindings/ScriptGroup';
  import type { ScriptReview } from '$lib/ipc/bindings/ScriptReview';
  import type { ScriptScope } from '$lib/ipc/bindings/ScriptScope';
  import type { ScriptSummary } from '$lib/ipc/bindings/ScriptSummary';
  import { deleteScript, type ReviewedScript, SCRIPTS_PAGE_SIZE, scriptDetail, scriptList, trustScripts } from '$lib/stores/memoryScripts.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { formatDateTime, relativeTime } from '$lib/utils/format';
  import { clampPage, pageCount } from '$lib/utils/pagination';

  import SessionConfirm, { type ConfirmRequest } from '../sessions/SessionConfirm.svelte';
  import ApprovalBadge from './ApprovalBadge.svelte';
  import MemoryFilterHint from './MemoryFilterHint.svelte';
  import MemoryScopeFilter from './MemoryScopeFilter.svelte';
  import MemorySearch from './MemorySearch.svelte';
  import MemoryTable from './MemoryTable.svelte';
  import ScriptPanel from './ScriptPanel.svelte';
  import { languageLabel, needsReview, PLACE_LABELS, scopeText } from './script-format';
  import { scriptColumns } from './script-table';

  type GroupChoice = ScriptGroup | 'all';
  type StatusChoice = ScriptReview | 'all';

  interface Shown {
    script: ScriptSummary | null;
    body: ScriptDetail | null;
  }

  /** How many script names the confirmation to trust several lists before it says "and n more". */
  const NAMED_IN_CONFIRM = 5;

  let confirmer: SessionConfirm | undefined = $state();
  let trusting = $state(false);

  const path = $derived(project.active?.path ?? null);
  const page = $derived(scriptList.data);
  const availability = $derived(page && page.availability.kind !== 'available' ? page.availability : null);
  const rows = $derived(page?.scripts ?? []);
  const total = $derived(page?.total ?? 0);
  const groups = $derived(page?.groups ?? { needsReview: 0, repo: 0, global: 0 });
  const reviews = $derived(page?.review ?? null);
  // Every script applies to one scope, so the two scope groups together are all of them.
  const everything = $derived(groups.repo + groups.global);
  // An older Harness Memory cannot combine the scope with the status: it gets one "Show" filter of its groups, and a hint that an update adds the scope.
  const combined = $derived(page?.combinedFilters ?? true);
  const searching = $derived(viewState.scriptsQuery.trim() !== '');
  const filtered = $derived(combined ? viewState.scriptsScope !== null || viewState.scriptsStatus !== null : viewState.scriptsGroup !== null);
  // Nothing saved yet: one page with the explanation, not an empty table beside a message.
  const none = $derived(page !== null && availability === null && total === 0 && !searching && !filtered);
  // Trusting several at once is for the scripts on screen: the ones of the page the user is looking at.
  const toTrust = $derived(rows.filter(needsReview));

  const statusOptions = $derived([
    { value: 'all' as StatusChoice, label: t('memory.filter.all'), count: reviews ? reviews.needed + reviews.trusted : undefined },
    { value: 'needed' as StatusChoice, label: t('memory.scripts.review.filter'), count: reviews?.needed },
    { value: 'trusted' as StatusChoice, label: t('memory.filter.trusted'), count: reviews?.trusted },
  ]);

  const groupOptions = $derived([
    { value: 'all' as GroupChoice, label: t('memory.filter.all'), count: everything },
    { value: 'needsReview' as GroupChoice, label: t('memory.scripts.review.filter'), count: groups.needsReview },
    { value: 'repo' as GroupChoice, label: t('memory.scripts.chips.repo'), count: groups.repo },
    { value: 'global' as GroupChoice, label: t('memory.scripts.row.global'), count: groups.global },
  ]);

  // A search that finds nothing says so. So do filters that leave nothing: they stay chosen, with the way to clear them in the row above.
  const noMatch = $derived.by(() => {
    if (page === null || rows.length > 0 || total > 0) {
      return null;
    }

    return !searching && filtered ? t('memory.scripts.list.noFilter') : t('memory.scripts.list.noMatch');
  });

  function chooseScope(scope: ScriptScope | null): void {
    viewState.scriptsScope = scope;
    viewState.scriptsPage = 0;
  }

  function chooseStatus(status: ScriptReview | null): void {
    viewState.scriptsStatus = status;
    viewState.scriptsPage = 0;
  }

  function chooseGroup(group: ScriptGroup | null): void {
    viewState.scriptsGroup = group;
    viewState.scriptsPage = 0;
  }

  // The open script. Its row is on this page only sometimes (a page turned), so the read of it also gives the facts.
  // The body belongs to it only: a read that is still on its way for the previous one never shows.
  const openId = $derived(viewState.scriptId);
  const body = $derived<ScriptDetail | null>(scriptDetail.data && scriptDetail.data.id === openId ? scriptDetail.data : null);
  const live = $derived<Shown>({ script: rows.find((row) => row.id === openId) ?? body, body });
  // Trust pins the body that is on screen: it needs the code read and the same version the list reports.
  const reviewed = $derived<ReviewedScript | null>(live.script && body && body.sha256 === live.script.sha256 ? { id: live.script.id, name: live.script.name, sha256: body.sha256 } : null);

  // A panel that closes slides out showing the script it had, not an empty frame.
  let last = $state.raw<Shown>({ script: null, body: null });
  const shown = $derived(openId === null ? last : live);

  $effect(() => {
    if (openId !== null && live.script) {
      last = live;
    }
  });

  $effect(() => {
    const id = openId;
    const where = path;

    untrack(() => scriptDetail.sync(id === null ? null : String(id), where));
  });

  // A script that an agent deleted while its panel was open is no longer there to show.
  $effect(() => {
    if (scriptDetail.status === 'error' && scriptDetail.error?.code === 'notFound') {
      viewState.scriptId = null;
    }
  });

  // A script an agent rewrote keeps its id: the body on screen is read again once the list reports a new version.
  // Each version is asked for once, so a server that answers with another one cannot start a loop.
  let reread = '';

  $effect(() => {
    const version = rows.find((row) => row.id === openId)?.sha256;

    if (version && body && body.sha256 !== version && version !== reread) {
      reread = version;
      untrack(() => void scriptDetail.refresh({ quiet: true }));
    }
  });

  // A new search starts at the first page.
  let searched = viewState.scriptsQuery;

  $effect(() => {
    const query = viewState.scriptsQuery;

    if (query !== searched) {
      searched = query;
      viewState.scriptsPage = 0;
    }
  });

  // Deleting the last script of a page leaves the pager past the end.
  $effect(() => {
    if (page && total > 0 && viewState.scriptsPage >= pageCount(total, SCRIPTS_PAGE_SIZE)) {
      viewState.scriptsPage = clampPage(viewState.scriptsPage, total, SCRIPTS_PAGE_SIZE);
    }
  });

  function ask(request: ConfirmRequest): Promise<boolean> {
    return confirmer?.ask(request) ?? Promise.resolve(false);
  }

  async function trust(scripts: ReviewedScript[]): Promise<void> {
    trusting = true;

    try {
      await trustScripts(scripts, path);
    } finally {
      trusting = false;
    }
  }

  /** Trusting several scripts at once is the one place a click can approve code unread, so it names how many and which. */
  async function trustShown(): Promise<void> {
    // The versions listed now are the ones trusted: a script rewritten while the question is open is refused, not approved unseen.
    const batch = toTrust.map(({ id, name, sha256 }) => ({ id, name, sha256 }));
    const named = batch.slice(0, NAMED_IN_CONFIRM).map((script) => script.name);
    const more = batch.length - named.length;

    const confirmed = await ask({
      title: tn('memory.scripts.trustAll.title', batch.length),
      description: t('memory.scripts.trustAll.body'),
      subject: {
        icon: ShieldCheck,
        name: tn('memory.scripts.count', batch.length),
        details: more > 0 ? `${named.join(', ')} ${t('memory.scripts.trustAll.more', { count: more })}` : named.join(', '),
      },
      confirmLabel: tn('memory.scripts.trustAll.confirm', batch.length),
      cancelLabel: t('memory.scripts.trustAll.cancel'),
    });

    if (confirmed) {
      await trust(batch);
    }
  }

  function remove(script: ScriptSummary): void {
    void deleteScript(script.id, path, () =>
      ask({
        title: t('memory.scripts.delete.title'),
        description: t('memory.scripts.delete.body'),
        subject: {
          icon: FileCode,
          name: script.name,
          details: [languageLabel(script.language), scopeText(script), t(PLACE_LABELS[script.place]), relativeTime(script.updatedAt)].join(' · '),
          note: script.place === 'team' ? t('memory.scripts.delete.teamNote') : undefined,
        },
        confirmLabel: t('memory.scripts.delete.confirm'),
      }),
    );
  }
</script>

{#snippet filters()}
  {#if combined}
    <MemoryScopeFilter value={viewState.scriptsScope} counts={page?.scopes ?? null} onchange={chooseScope} />

    <FilterMenu
      label={t('memory.filter.status')}
      prefix={t('memory.filter.status')}
      options={statusOptions}
      value={viewState.scriptsStatus ?? 'all'}
      onchange={(next) => chooseStatus(next === 'all' ? null : next)}
      onclear={() => chooseStatus(null)}
      clearLabel={t('memory.filter.clearStatus')}
      align="start"
    />
  {:else}
    <FilterMenu
      label={t('memory.filter.show')}
      prefix={t('memory.filter.show')}
      options={groupOptions}
      value={viewState.scriptsGroup ?? 'all'}
      onchange={(next) => chooseGroup(next === 'all' ? null : next)}
      onclear={() => chooseGroup(null)}
      clearLabel={t('memory.filter.clearShow')}
      align="start"
    />
  {/if}
{/snippet}

{#snippet hint()}
  <MemoryFilterHint />
{/snippet}

{#if page && !availability && !none}
  <MemorySearch bind:value={viewState.scriptsQuery} label={t('memory.scripts.list.search')} placeholder={t('memory.scripts.list.searchPlaceholder')}>
    {#snippet before()}
      {#if toTrust.length > 0}
        <Button variant="ghost" loading={trusting} onclick={() => void trustShown()}>
          {#snippet leading()}<ShieldCheck size={ICON.sm} aria-hidden="true" />{/snippet}
          {t('memory.scripts.trustAll.action')}
        </Button>
      {/if}
    {/snippet}
  </MemorySearch>
{/if}

<MemoryTable
  loader={scriptList}
  loaded={page !== null}
  {availability}
  {none}
  emptyIcon={FileCode}
  emptyTitle={t('memory.scripts.empty.none.title')}
  emptyBody={t('memory.scripts.empty.none.body')}
  failedTitle={t('memory.scripts.empty.failed.title')}
  loadingLabel={t('memory.scripts.loading')}
  {noMatch}
  count={tn('memory.scripts.count', total)}
  {filters}
  hint={combined ? undefined : hint}
  detailEmpty={t('memory.scripts.detail.empty')}
  tableLabel={t('memory.scripts.list.label')}
  tableId="memory-scripts"
  columns={scriptColumns()}
  {rows}
  sort={viewState.scriptsSort}
  onsort={(next) => {
    viewState.scriptsSort = next;
    viewState.scriptsPage = 0;
  }}
  page={viewState.scriptsPage}
  pageSize={SCRIPTS_PAGE_SIZE}
  {total}
  onpage={(next) => (viewState.scriptsPage = next)}
  {openId}
  onopen={(id) => (viewState.scriptId = id)}
>
  {#snippet cell(script: ScriptSummary, column: Column)}
    {#if column.key === 'name'}
      <span class="flex min-w-0 items-center gap-2">
        <span class="min-w-0 truncate font-mono text-fg">{script.name}</span>
        {#if script.approval !== 'local'}<ApprovalBadge approval={script.approval} />{/if}
      </span>
    {:else if column.key === 'language'}
      <Badge>{languageLabel(script.language)}</Badge>
    {:else if column.key === 'scope'}
      <span class="text-fg-2" title={script.repo ?? undefined}>{scopeText(script)}</span>
    {:else if column.key === 'place'}
      <span class="text-fg-2">{t(PLACE_LABELS[script.place])}</span>
    {:else if column.key === 'runs'}
      <span class="tabular text-fg-2">{script.runCount}</span>
    {:else if column.key === 'lastRun'}
      {#if script.lastRunAt}
        <span class="tabular text-fg-3" title={formatDateTime(script.lastRunAt)}>{relativeTime(script.lastRunAt)}</span>
      {:else}
        <span class="text-fg-3">{t('memory.scripts.detail.neverRun')}</span>
      {/if}
    {:else}
      <span class="tabular text-fg-3" title={formatDateTime(script.updatedAt)}>{relativeTime(script.updatedAt)}</span>
    {/if}
  {/snippet}

  {#snippet panel({ close, inline })}
    <ScriptPanel
      open={openId !== null}
      {inline}
      script={shown.script}
      detail={shown.body}
      status={scriptDetail.status}
      error={scriptDetail.error?.message ?? null}
      onretry={() => void scriptDetail.refresh()}
      ontrust={reviewed ? () => void trust([reviewed]) : undefined}
      {trusting}
      ondelete={() => shown.script && remove(shown.script)}
      onclose={close}
    />
  {/snippet}
</MemoryTable>

<SessionConfirm bind:this={confirmer} />
