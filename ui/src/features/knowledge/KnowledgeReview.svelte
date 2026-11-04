<script lang="ts">
  import ShieldCheck from '@lucide/svelte/icons/shield-check';
  import { untrack } from 'svelte';

  import AuthorCell from '$lib/components/AuthorCell.svelte';
  import Badge from '$lib/components/Badge.svelte';
  import type { ScopeChoice } from '$lib/components/scope/scope-options';
  import ScopeFilter from '$lib/components/scope/ScopeFilter.svelte';
  import type { Column } from '$lib/components/table';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeHeldItem } from '$lib/ipc/bindings/KnowledgeHeldItem';
  import { KNOWLEDGE_PAGE_SIZE } from '$lib/stores/knowledgeDocs.svelte';
  import { followUpKey, knowledgeHeld } from '$lib/stores/knowledgeHeld.svelte';
  import { projectScopes } from '$lib/stores/project-scopes.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { formatDateTime, relativeTime } from '$lib/utils/format';
  import { clampPage, pageCount } from '$lib/utils/pagination';

  import MemorySearch from '../memory/MemorySearch.svelte';
  import MemoryTable from '../memory/MemoryTable.svelte';
  import HeldPanel from './HeldPanel.svelte';
  import { folderHint, folderName, heldColumns, linksText, proposerOf } from './knowledge-format';
  import { knowledgeScopeOptions } from './knowledge-repos';
  import KnowledgeEmpty from './KnowledgeEmpty.svelte';
  import KnowledgeTabs from './KnowledgeTabs.svelte';
  import { exitRow } from './row-exit';

  const page = $derived(knowledgeHeld.list.data);
  const rows = $derived(page?.items ?? []);
  // `total` of the page counts the whole queue, whatever the search: it is what the badge says.
  const queue = $derived(page?.total ?? 0);
  /** The projects on this computer, all and global: the queue is small, so no counts per repository. */
  const scopeOptions = $derived(knowledgeScopeOptions(projectScopes.data ?? [], null, queue));
  /** A scope or a search narrows the queue: an empty answer then is "no match", with the filters still there to undo it. */
  const filtered = $derived(viewState.heldScope !== 'all' || viewState.heldQuery.trim() !== '');
  const none = $derived(page !== null && queue === 0 && !filtered);
  const noMatch = $derived(page !== null && rows.length === 0 && (queue > 0 || filtered) ? t('knowledge.review.noMatch') : null);

  const openId = $derived(viewState.heldId);
  const detail = $derived(knowledgeHeld.detail.data && knowledgeHeld.detail.data.id === openId ? knowledgeHeld.detail.data : null);
  const live = $derived<KnowledgeHeldItem | null>(rows.find((row) => row.id === openId) ?? detail);

  // A panel that closes slides out showing the proposal it had.
  let last = $state.raw<KnowledgeHeldItem | null>(null);
  const shown = $derived(openId === null ? last : live);

  $effect(() => {
    if (openId !== null && live) {
      last = live;
    }
  });

  // A proposal somebody decided meanwhile (another window, the CLI) is gone from the queue.
  $effect(() => {
    if (knowledgeHeld.detail.status === 'error' && knowledgeHeld.detail.error?.code === 'notFound') {
      viewState.heldId = null;
    }
  });

  $effect(() => {
    if (page && queue > 0 && viewState.heldPage >= pageCount(queue, KNOWLEDGE_PAGE_SIZE)) {
      untrack(() => (viewState.heldPage = clampPage(viewState.heldPage, queue, KNOWLEDGE_PAGE_SIZE)));
    }
  });

  // Whether the chat of the open proposal's agent is open in this app: only then can "Request changes" reach the agent.
  $effect(() => {
    const key = shown ? followUpKey(shown) : null;

    untrack(() => knowledgeHeld.followUp.sync(key, null));
  });

  const followUp = $derived(shown?.session != null && knowledgeHeld.followUp.data === true);

  /** Lets the row go, asks the server, and brings the row back when the decision did not take it away. */
  async function approve(item: KnowledgeHeldItem): Promise<void> {
    viewState.heldId = null;

    const restore = await exitRow(item.id);

    if ((await knowledgeHeld.decide(item, 'approve')) === 'stay') {
      restore();
    }
  }

  /** The proposal stays in the queue, marked as waiting for its agent's revision. */
  async function request(item: KnowledgeHeldItem, comment: string): Promise<void> {
    await knowledgeHeld.requestChanges(item, comment);
  }

  /** The row leaves at once and a toast offers to undo: the server is asked when the toast is gone. */
  async function reject(item: KnowledgeHeldItem, reason: string | null): Promise<void> {
    viewState.heldId = null;

    const restore = await exitRow(item.id);

    knowledgeHeld.rejectWithUndo(item, reason, restore);
  }
</script>

{#if none}
  <KnowledgeEmpty icon={ShieldCheck} title={t('knowledge.review.empty.title')} body={t('knowledge.review.empty.body')} />
{:else}
  {#if page}
    <MemorySearch bind:value={viewState.heldQuery} label={t('knowledge.review.search.label')} placeholder={t('knowledge.review.search.placeholder')} />
  {/if}

  <MemoryTable
    loader={knowledgeHeld.list}
    loaded={page !== null}
    availability={null}
    none={false}
    emptyIcon={ShieldCheck}
    emptyTitle={t('knowledge.review.empty.title')}
    emptyBody={t('knowledge.review.empty.body')}
    failedTitle={t('knowledge.review.failed')}
    loadingLabel={t('knowledge.review.loading')}
    {noMatch}
    tableLabel={t('knowledge.review.table.label')}
    tableId="knowledge-review"
    columns={heldColumns()}
    {rows}
    sort={viewState.heldSort}
    onsort={(next) => {
      viewState.heldSort = next;
      viewState.heldPage = 0;
    }}
    page={viewState.heldPage}
    pageSize={KNOWLEDGE_PAGE_SIZE}
    total={queue}
    onpage={(next) => (viewState.heldPage = next)}
    {openId}
    onopen={(id) => (viewState.heldId = id)}
  >
    {#snippet filters()}
      <KnowledgeTabs onchange={() => (viewState.heldId = null)} />

      <ScopeFilter
        options={scopeOptions}
        value={viewState.heldScope as ScopeChoice}
        onchange={(next) => {
          viewState.heldScope = next;
          viewState.heldPage = 0;
          viewState.heldId = null;
        }}
      />
    {/snippet}

    {#snippet cell(item: KnowledgeHeldItem, column: Column)}
      {#if column.key === 'title'}
        <span class="text-fg">{item.title}</span>
        {#if item.status === 'changesRequested'}<span class="ml-2 inline-block align-middle"><Badge tone="danger">{t('knowledge.status.changesRequested')}</Badge></span>{/if}
      {:else if column.key === 'addedBy'}
        <AuthorCell author={proposerOf(item)} />
      {:else if column.key === 'project'}
        <span class="text-fg-2" title={item.projectPath === null ? linksText(item.links) : folderHint(item.projectPath)}>{folderName(item.projectPath) || linksText(item.links)}</span>
      {:else}
        <span class="tabular text-fg-3" title={formatDateTime(item.createdAt)}>{relativeTime(item.createdAt)}</span>
      {/if}
    {/snippet}

    {#snippet panel({ close })}
      <HeldPanel
        open={openId !== null}
        item={shown}
        {detail}
        status={knowledgeHeld.detail.status}
        error={knowledgeHeld.detail.error?.message ?? null}
        busy={shown !== null && knowledgeHeld.busy.has(shown.id)}
        {followUp}
        onretry={() => void knowledgeHeld.detail.refresh()}
        onapprove={(item) => void approve(item)}
        onrequest={(item, comment) => void request(item, comment)}
        onreject={(item, reason) => void reject(item, reason)}
        onclose={close}
      />
    {/snippet}
  </MemoryTable>
{/if}

