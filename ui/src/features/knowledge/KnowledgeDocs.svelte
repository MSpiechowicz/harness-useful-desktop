<script lang="ts">
  import FileText from '@lucide/svelte/icons/file-text';
  import FolderUp from '@lucide/svelte/icons/folder-up';
  import Globe from '@lucide/svelte/icons/globe';
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import Library from '@lucide/svelte/icons/library';
  import { untrack } from 'svelte';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import DocNewButton from '$lib/components/docs/DocNewButton.svelte';
  import FilterMenu from '$lib/components/FilterMenu.svelte';
  import { ICON } from '$lib/components/icon';
  import ScopeFilter from '$lib/components/scope/ScopeFilter.svelte';
  import { choiceKey, type ScopeChoice } from '$lib/components/scope/scope-options';
  import type { Column } from '$lib/components/table';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeDocument } from '$lib/ipc/bindings/KnowledgeDocument';
  import type { KnowledgeLink } from '$lib/ipc/bindings/KnowledgeLink';
  import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
  import { knowledgeDocs, KNOWLEDGE_PAGE_SIZE } from '$lib/stores/knowledgeDocs.svelte';
  import { knowledgeImport } from '$lib/stores/knowledgeImport.svelte';
  import { projectScopes } from '$lib/stores/project-scopes.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { formatDateTime, relativeTime } from '$lib/utils/format';
  import { clampPage, pageCount } from '$lib/utils/pagination';

  import MemorySearch from '../memory/MemorySearch.svelte';
  import MemoryTable from '../memory/MemoryTable.svelte';
  import SessionConfirm from '../sessions/SessionConfirm.svelte';
  import DocumentPanel from './DocumentPanel.svelte';
  import { docColumns, docSubtitle, formatKey, formatLabel, formatTone, KNOWLEDGE_FORMATS, linksText, pathCount, pathsText, repoLabel } from './knowledge-format';
  import { knowledgeScopeOptions } from './knowledge-repos';
  import KnowledgeEmpty from './KnowledgeEmpty.svelte';
  import KnowledgeTabs from './KnowledgeTabs.svelte';
  import { exitRow } from './row-exit';

  let confirmer: SessionConfirm | undefined = $state();

  const page = $derived(knowledgeDocs.list.data);
  const rows = $derived(page?.items ?? []);
  const total = $derived(page?.total ?? 0);
  const searching = $derived(viewState.knowledgeQuery.trim() !== '');
  const filtered = $derived(viewState.knowledgeScope !== 'all' || viewState.knowledgeFormat !== null);
  // Nothing at all is kept: no documents, and no search or filter that would explain it.
  const none = $derived(page !== null && total === 0 && !searching && !filtered);
  const options = $derived(knowledgeScopeOptions(projectScopes.data ?? [], knowledgeDocs.repos.data, knowledgeChanges.pulse?.documents));

  const formatOptions = $derived([
    { value: 'all', label: t('memory.filter.all') },
    ...KNOWLEDGE_FORMATS.map((format) => ({ value: format as string, label: formatLabel(format) })),
  ]);

  const noMatch = $derived.by(() => {
    if (page === null || rows.length > 0 || total > 0) {
      return null;
    }

    return !searching && filtered ? t('knowledge.docs.noFilter') : t('knowledge.docs.noMatch');
  });

  // The document whose panel is open. Its row is on this page only sometimes (a page turned, a filter), so the read of it also gives the facts.
  const openId = $derived(viewState.knowledgeDocId);
  const detail = $derived(knowledgeDocs.detail.data && knowledgeDocs.detail.data.document.id === openId ? knowledgeDocs.detail.data : null);
  const live = $derived<KnowledgeDocument | null>(detail?.document ?? rows.find((row) => row.id === openId) ?? null);

  // A panel that closes slides out showing the document it had, not an empty frame.
  let last = $state.raw<KnowledgeDocument | null>(null);
  const shown = $derived(openId === null ? last : live);

  $effect(() => {
    if (openId !== null && live) {
      last = live;
    }
  });

  // A document that was deleted (here or by an agent) while its panel was open is no longer there to show.
  $effect(() => {
    if (knowledgeDocs.detail.status === 'error' && knowledgeDocs.detail.error?.code === 'notFound') {
      viewState.knowledgeDocId = null;
    }
  });

  // Deleting the last document of a page leaves the pager past the end.
  $effect(() => {
    if (page && total > 0 && viewState.knowledgePage >= pageCount(total, KNOWLEDGE_PAGE_SIZE)) {
      viewState.knowledgePage = clampPage(viewState.knowledgePage, total, KNOWLEDGE_PAGE_SIZE);
    }
  });

  // A repository that has no document any more is let go of, instead of leaving a filter nothing shows. A project on this computer stays chosen.
  $effect(() => {
    const choice = viewState.knowledgeScope;
    const known = knowledgeDocs.repos.data;

    if (choiceKey(choice as ScopeChoice) !== null && known?.availability.type === 'available' && projectScopes.data !== null && !options.some((option) => option.value === choice)) {
      untrack(() => (viewState.knowledgeScope = 'all'));
      viewState.knowledgePage = 0;
    }
  });

  function choose(change: () => void): void {
    change();
    viewState.knowledgePage = 0;
    viewState.knowledgeDocId = null;
  }

  async function rename(title: string): Promise<boolean> {
    return openId !== null && (await knowledgeDocs.rename(openId, title)) !== null;
  }

  async function relink(links: KnowledgeLink[]): Promise<KnowledgeLink[] | null> {
    return openId === null ? null : knowledgeDocs.setLinks(openId, links);
  }

  async function remove(): Promise<void> {
    const doc = shown;

    if (!doc) {
      return;
    }

    const confirmed = await confirmer?.ask({
      title: t('knowledge.delete.title'),
      description: t('knowledge.delete.body'),
      subject: { icon: FileText, name: doc.title, details: [docSubtitle(doc), relativeTime(doc.updatedAt)].join(' · ') },
      confirmLabel: t('knowledge.panel.delete'),
    });

    if (!confirmed) {
      return;
    }

    viewState.knowledgeDocId = null;

    const restore = await exitRow(doc.id);
    const result = await knowledgeDocs.remove([doc.id]);

    if (result === null) {
      restore();
    } else {
      toasts.push({ title: t('knowledge.toast.deleted'), description: doc.title, tone: 'success' });
    }
  }

  function addFolder(): void {
    knowledgeImport.begin();
    void knowledgeImport.pick('folder');
  }
</script>

{#snippet filters()}
  <KnowledgeTabs onchange={() => (viewState.knowledgeDocId = null)} />

  <ScopeFilter {options} value={viewState.knowledgeScope as ScopeChoice} onchange={(next) => choose(() => {
        viewState.knowledgeScope = next;
      })} disableKeyless />

  <FilterMenu
    label={t('knowledge.filter.format')}
    prefix={t('knowledge.filter.format')}
    options={formatOptions}
    value={viewState.knowledgeFormat ?? 'all'}
    onchange={(next) => choose(() => {
        viewState.knowledgeFormat = next === 'all' ? null : next;
      })}
    onclear={() => choose(() => {
        viewState.knowledgeFormat = null;
      })}
    clearLabel={t('knowledge.filter.clearFormat')}
    align="start"
  />
{/snippet}

{#snippet emptyAction()}
  <div class="flex flex-wrap items-center justify-center gap-2">
    <DocNewButton label={t('knowledge.add')} onclick={() => knowledgeImport.begin()} />

    <Button onclick={addFolder}>
      {#snippet leading()}<FolderUp size={ICON.sm} aria-hidden="true" />{/snippet}
      {t('knowledge.docs.empty.folder')}
    </Button>
  </div>
{/snippet}

{#if none}
  <KnowledgeEmpty icon={Library} title={t('knowledge.docs.empty.title')} body={t('knowledge.docs.empty.body')} action={emptyAction} />
{:else}
  {#if page}
    <MemorySearch bind:value={viewState.knowledgeQuery} label={t('knowledge.search.label')} placeholder={t('knowledge.search.placeholder')} />
  {/if}

<MemoryTable
  loader={knowledgeDocs.list}
  loaded={page !== null}
  availability={null}
  none={false}
  emptyIcon={Library}
  emptyTitle={t('knowledge.docs.empty.title')}
  emptyBody={t('knowledge.docs.empty.body')}
  failedTitle={t('knowledge.docs.failed')}
  loadingLabel={t('knowledge.docs.loading')}
  {noMatch}
  {filters}
  detailEmpty={t('knowledge.docs.detail.empty')}
  tableLabel={t('knowledge.table.label')}
  tableId="knowledge-docs"
  columns={docColumns()}
  {rows}
  sort={viewState.knowledgeSort}
  onsort={(next) => choose(() => {
    viewState.knowledgeSort = next;
  })}
  page={viewState.knowledgePage}
  pageSize={KNOWLEDGE_PAGE_SIZE}
  {total}
  onpage={(next) => (viewState.knowledgePage = next)}
  {openId}
  onopen={(id) => (viewState.knowledgeDocId = id)}
>
  {#snippet cell(doc: KnowledgeDocument, column: Column)}
    {#if column.key === 'title'}
      <span class="text-fg">{doc.title}</span>
    {:else if column.key === 'format'}
      {@const format = formatKey(doc)}
      <Badge tone={formatTone(format)} size="lg">{formatLabel(format)}</Badge>
    {:else if column.key === 'repos'}
      {@render repos(doc.links)}
    {:else if column.key === 'chunks'}
      <span class="tabular text-fg-2">{doc.chunkCount}</span>
    {:else}
      <span class="tabular text-fg-3" title={formatDateTime(doc.updatedAt)}>{relativeTime(doc.updatedAt)}</span>
    {/if}
  {/snippet}

  {#snippet panel({ close, inline })}
    <DocumentPanel
      open={openId !== null}
      {inline}
      doc={shown}
      {detail}
      status={knowledgeDocs.detail.status}
      error={knowledgeDocs.detail.error?.message ?? null}
      {options}
      onretry={() => void knowledgeDocs.detail.refresh()}
      onrename={rename}
      onlinks={relink}
      ondelete={() => void remove()}
      onclose={close}
    />
  {/snippet}
</MemoryTable>
{/if}

<SessionConfirm bind:this={confirmer} />

<!-- Where a document applies: Global, or the first repository with how many more and how many paths it is narrowed to. -->
{#snippet repos(links: KnowledgeLink[])}
  {#if links.length === 0}
    <span class="flex items-center gap-2 text-fg-2"><Globe size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />{t('knowledge.repos.global')}</span>
  {:else}
    {@const paths = pathsText(pathCount(links))}
    <span class="flex min-w-0 items-center gap-2 text-fg-2" title={linksText(links)}>
      <GitBranch size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />
      <span class="truncate">{repoLabel(links[0].repo)}</span>
      {#if links.length > 1}<span class="shrink-0 text-xs text-fg-3">{t('knowledge.repos.more', { count: links.length - 1 })}</span>{/if}
      {#if paths}<span class="shrink-0 text-xs text-fg-3">{paths}</span>{/if}
    </span>
  {/if}
{/snippet}
