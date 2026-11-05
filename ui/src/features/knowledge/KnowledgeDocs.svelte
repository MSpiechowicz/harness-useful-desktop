<script lang="ts">
  import FileText from '@lucide/svelte/icons/file-text';
  import FolderUp from '@lucide/svelte/icons/folder-up';
  import Globe from '@lucide/svelte/icons/globe';
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import BookOpenText from '@lucide/svelte/icons/book-open-text';
  import { untrack } from 'svelte';

  import AuthorCell from '$lib/components/AuthorCell.svelte';
  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import DocNewButton from '$lib/components/docs/DocNewButton.svelte';
  import FilterMenu from '$lib/components/FilterMenu.svelte';
  import { ICON } from '$lib/components/icon';
  import ScopeFilter from '$lib/components/scope/ScopeFilter.svelte';
  import { choiceKey, type ScopeChoice } from '$lib/components/scope/scope-options';
  import type { Column } from '$lib/components/table';
  import UsefulCell from '$lib/components/UsefulCell.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeDocument } from '$lib/ipc/bindings/KnowledgeDocument';
  import type { KnowledgeLink } from '$lib/ipc/bindings/KnowledgeLink';
  import type { KnowledgeStatusFilter } from '$lib/ipc/bindings/KnowledgeStatusFilter';
  import type { KnowledgeUsefulFilter } from '$lib/ipc/bindings/KnowledgeUsefulFilter';
  import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
  import { knowledgeDocs, KNOWLEDGE_PAGE_SIZE } from '$lib/stores/knowledgeDocs.svelte';
  import { knowledgeImport } from '$lib/stores/knowledgeImport.svelte';
  import { knowledgeRemote } from '$lib/stores/knowledgeRemote.svelte';
  import { knowledgeReplace } from '$lib/stores/knowledgeReplace.svelte';
  import { knowledgeReview } from '$lib/stores/knowledgeReview.svelte';
  import { knowledgeStats } from '$lib/stores/knowledgeStats.svelte';
  import { knowledgeStatus } from '$lib/stores/knowledgeStatus.svelte';
  import { projectScopes } from '$lib/stores/project-scopes.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { authorLine } from '$lib/utils/author-format';
  import { formatBytes, formatDateTime, relativeTime } from '$lib/utils/format';
  import { readFailureText } from '$lib/utils/knowledge-failure';
  import { usesBothPlaces } from '$lib/utils/knowledge-routing';
  import { clampPage, pageCount } from '$lib/utils/pagination';

  import MemorySearch from '../memory/MemorySearch.svelte';
  import MemoryTable from '../memory/MemoryTable.svelte';
  import { usefulCounts } from '../memory/note-format';
  import SessionConfirm from '../sessions/SessionConfirm.svelte';
  import DocumentPanel from './DocumentPanel.svelte';
  import { docColumns, docSubtitle, formatKey, formatLabel, formatTone, KNOWLEDGE_FORMATS, linksText, pathCount, placeLabel, repoLabel } from './knowledge-format';
  import { keptRemote } from './knowledge-replace';
  import { lookOf, STATUS_CHOICE_LABELS, STATUS_CHOICES } from './knowledge-status';
  import { knowledgeScopeOptions } from './knowledge-repos';
  import KnowledgeEmpty from './KnowledgeEmpty.svelte';
  import KnowledgePlaceFilter from './KnowledgePlaceFilter.svelte';
  import KnowledgeTabs from './KnowledgeTabs.svelte';
  import { takeStep } from './review-steps';
  import { decisionPin, type ReviewStep, versionOf } from './review-model';
  import { exitRow } from './row-exit';

  type UsefulChoice = KnowledgeUsefulFilter | 'all';

  let confirmer: SessionConfirm | undefined = $state();

  const page = $derived(knowledgeDocs.list.data);
  const rows = $derived(page?.items ?? []);
  const total = $derived(page?.total ?? 0);
  const searching = $derived(viewState.knowledgeQuery.trim() !== '');
  const filtered = $derived(viewState.knowledgeScope !== 'all' || viewState.knowledgeFormat !== null || viewState.knowledgePlace !== null || viewState.knowledgeUseful !== null || viewState.knowledgeStatus !== null);
  // Nothing at all is kept: no documents, and no search or filter that would explain it.
  const none = $derived(page !== null && total === 0 && !searching && !filtered);
  // Both places are in use while the routing says so, or while a document says where it is kept. The routing is known once it was read.
  const routing = $derived(knowledgeRemote.value);
  const bothPlaces = $derived((routing !== null && usesBothPlaces(routing.enabled, routing)) || rows.some((row) => row.place !== undefined));
  // The server narrows a list to one place only with `offline-fallback`: without it the filter would change nothing, so it is not offered.
  const canNarrow = $derived(knowledgeStatus.value?.features.offlineFallback === true);
  const options = $derived(knowledgeScopeOptions(projectScopes.data ?? [], knowledgeDocs.repos.data, knowledgeChanges.pulse?.documents));

  // A Harness Knowledge that counts how often agents found a document useful has the two columns and the Usefulness filter, and one that tells who added a document its "Added by" column.
  // Which it is says the server, and until it was asked the rows do.
  const features = $derived(knowledgeStatus.value?.features);
  const usefulListed = $derived(features?.docsUseful ?? rows.some((row) => row.shown != null));
  const authorsListed = $derived(features?.docsAuthors ?? rows.some((row) => row.addedBy != null));
  // A Harness Knowledge that keeps drafts has the Status column and the Status filter, and the panel's review, changes and history.
  // They stay while a status filter is applied, even when no row on the page shows a state (the filter answered nothing), so the user can see and clear it.
  const draftsListed = $derived((features?.docsDrafts ?? rows.some((row) => row.status !== 'live')) || viewState.knowledgeStatus !== null);

  const statusOptions = $derived(STATUS_CHOICES.map((value) => ({ value, label: t(STATUS_CHOICE_LABELS[value]) })));

  const usefulOptions = $derived([
    { value: 'all' as UsefulChoice, label: t('memory.filter.all') },
    { value: 'ever' as UsefulChoice, label: t('knowledge.filter.useful.ever') },
    { value: 'never' as UsefulChoice, label: t('knowledge.filter.useful.never'), hint: t('knowledge.filter.useful.neverHint') },
  ]);

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

  // The routing went back to one place, or the server cannot narrow: there is nothing to narrow to any more.
  $effect(() => {
    if (viewState.knowledgePlace !== null && ((routing !== null && !bothPlaces) || (knowledgeStatus.value !== null && !canNarrow))) {
      viewState.knowledgePlace = null;
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
    return openId === null ? null : knowledgeDocs.setLinks(openId, links, shown?.title ?? '');
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

  // A read-only role on the remote place can neither replace nor delete what is kept there. While it is not known (no remote, a remote that is down) the actions stay on.
  const readOnly = $derived(shown !== null && keptRemote(shown, routing) && knowledgeStats.remoteWritable === false);

  /** Asks the user to confirm replacing the open document by `file`, and replaces it. With `ask` the replacement is also asked for approval, with the note for the reviewers. */
  async function replaceWith(source: 'pick' | 'drop', ask: { note: string } | null = null): Promise<void> {
    const doc = shown;

    if (!doc || readOnly || knowledgeReplace.busy) {
      return;
    }

    const file = await (source === 'pick' ? knowledgeReplace.pick() : knowledgeReplace.takeDrop());

    if (!file) {
      return;
    }

    const confirmed = await confirmer?.ask({
      title: t('knowledge.replace.title'),
      description: t('knowledge.replace.body', { name: file.name, size: formatBytes(file.bytes ?? 0) }),
      subject: { icon: FileText, name: doc.title, details: [formatLabel(formatKey(doc)), ...(doc.updatedBy ? [t('knowledge.replace.updatedBy', { name: authorLine(doc.updatedBy) })] : []), relativeTime(doc.updatedAt)].join(' · ') },
      confirmLabel: t(ask ? 'knowledge.drafts.replace.confirmAsk' : 'knowledge.replace.confirm'),
      cancelLabel: t('knowledge.replace.keep'),
      tone: 'primary',
    });

    if (confirmed && (await knowledgeReplace.replace(doc, file, ask))) {
      await knowledgeDocs.refresh();
    }
  }

  /** A button of the Review section: a step of the review, or choosing the file that replaces the version (with the note, asking at once). */
  function step(next: ReviewStep, text: string): void {
    const doc = shown;
    const version = doc ? versionOf(doc) : null;

    if (!doc || !version) {
      return;
    }

    if (next === 'replace') {
      void replaceWith('pick', { note: text });
    } else if (next === 'approve' || next === 'requestChanges') {
      // A verdict is about the diff on screen: it waits for that diff to be the version's latest text.
      const pin = decisionPin(knowledgeReview.shownCompare(version.id), version);

      if (pin.reason === null) {
        void takeStep(doc, version, next, text, pin.revision);
      }
    } else {
      void takeStep(doc, version, next, text);
    }
  }

  function download(liveVersion: boolean): void {
    if (shown) {
      void knowledgeReview.download({ id: shown.id, title: shown.title }, shown.sourceName, liveVersion);
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

  {#if canNarrow && (bothPlaces || viewState.knowledgePlace !== null)}
    <KnowledgePlaceFilter place={viewState.knowledgePlace} onchange={(next) => choose(() => {
          viewState.knowledgePlace = next;
        })} />
  {/if}

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

  {#if draftsListed}
    <FilterMenu
      label={t('knowledge.filter.status')}
      prefix={t('knowledge.filter.status')}
      options={statusOptions}
      value={viewState.knowledgeStatus ?? 'all'}
      onchange={(next) => choose(() => {
          viewState.knowledgeStatus = next === 'all' ? null : (next as KnowledgeStatusFilter);
        })}
      onclear={() => choose(() => {
          viewState.knowledgeStatus = null;
        })}
      clearLabel={t('knowledge.filter.clearStatus')}
      align="start"
      wide
    />
  {/if}

  {#if usefulListed || viewState.knowledgeUseful !== null}
    <FilterMenu
      label={t('knowledge.filter.useful')}
      prefix={t('knowledge.filter.useful')}
      options={usefulOptions}
      value={viewState.knowledgeUseful ?? 'all'}
      onchange={(next) => choose(() => {
          viewState.knowledgeUseful = next === 'all' ? null : next;
        })}
      onclear={() => choose(() => {
          viewState.knowledgeUseful = null;
        })}
      clearLabel={t('knowledge.filter.clearUseful')}
      align="start"
      wide
    />
  {/if}
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
  <KnowledgeEmpty icon={BookOpenText} title={t('knowledge.docs.empty.title')} body={t('knowledge.docs.empty.body')} action={emptyAction} />
{:else}
  {#if page}
    <MemorySearch bind:value={viewState.knowledgeQuery} label={t('knowledge.search.label')} placeholder={t('knowledge.search.placeholder')} />
  {/if}

<MemoryTable
  loader={knowledgeDocs.list}
  loaded={page !== null}
  availability={null}
  none={false}
  emptyIcon={BookOpenText}
  emptyTitle={t('knowledge.docs.empty.title')}
  emptyBody={t('knowledge.docs.empty.body')}
  failedTitle={t('knowledge.docs.failed')}
  loadingLabel={t('knowledge.docs.loading')}
  {noMatch}
  {filters}
  tableLabel={t('knowledge.table.label')}
  tableId="knowledge-docs"
  columns={docColumns(usefulListed, authorsListed, draftsListed, features?.docsSortPaths === true)}
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
      <!-- Only while both places are in use does a document say where it is kept. -->
      {#if doc.place}<span class="ml-2 inline-block align-middle"><Badge>{placeLabel(doc.place)}</Badge></span>{/if}
    {:else if column.key === 'format'}
      {@const format = formatKey(doc)}
      <Badge tone={formatTone(format)} size="lg">{formatLabel(format)}</Badge>
    {:else if column.key === 'status'}
      {@const look = lookOf(doc)}
      <Badge tone={look.tone} outline={look.outline} size="lg"><look.icon size={ICON.sm} aria-hidden="true" />{look.label}</Badge>
    {:else if column.key === 'repos'}
      {@render repos(doc.links)}
    {:else if column.key === 'paths'}
      <span class="tabular text-fg-2">{pathCount(doc.links)}</span>
    {:else if column.key === 'chunks'}
      <span class="tabular text-fg-2">{doc.chunkCount}</span>
    {:else if column.key === 'useful'}
      {@const counts = usefulCounts(doc)}
      {#if counts}<UsefulCell {counts} ratio={t('knowledge.useful.ratio', counts)} label={t('knowledge.useful.meter')} />{/if}
    {:else if column.key === 'lastUseful'}
      {#if doc.lastUsefulAt}
        <span class="tabular text-fg-3" title={formatDateTime(doc.lastUsefulAt)}>{relativeTime(doc.lastUsefulAt)}</span>
      {:else if usefulCounts(doc)}
        <span class="text-fg-3">{t('knowledge.useful.never')}</span>
      {/if}
    {:else if column.key === 'addedBy'}
      <AuthorCell author={doc.addedBy} />
    {:else}
      <span class="tabular text-fg-3" title={formatDateTime(doc.updatedAt)}>{relativeTime(doc.updatedAt)}</span>
    {/if}
  {/snippet}

  {#snippet panel({ close })}
    <DocumentPanel
      open={openId !== null}
      doc={shown}
      {detail}
      status={knowledgeDocs.detail.status}
      error={knowledgeDocs.detail.error ? readFailureText(knowledgeDocs.detail.error) : null}
      {options}
      query={viewState.knowledgeQuery}
      onretry={() => void knowledgeDocs.detail.refresh()}
      onrename={rename}
      onlinks={relink}
      ondelete={() => void remove()}
      onreplace={() => void replaceWith('pick')}
      onreplacedrop={() => void replaceWith('drop')}
      {readOnly}
      onstep={step}
      ondownload={download}
      onreview={(section) => {
        viewState.knowledgeReviewSection = section;
        viewState.knowledgeReviewId = openId;
      }}
      onclose={close}
    />
  {/snippet}
</MemoryTable>
{/if}

<SessionConfirm bind:this={confirmer} />

<!-- Where a document applies: Global, or the first repository with how many more. -->
{#snippet repos(links: KnowledgeLink[])}
  {#if links.length === 0}
    <span class="flex items-center gap-2 text-fg-2"><Globe size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />{t('knowledge.repos.global')}</span>
  {:else}
    <span class="flex min-w-0 items-center gap-2 text-fg-2" title={linksText(links)}>
      <GitBranch size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />
      <span class="truncate">{repoLabel(links[0].repo)}</span>
      {#if links.length > 1}<span class="shrink-0 text-xs text-fg-3">{t('knowledge.repos.more', { count: links.length - 1 })}</span>{/if}
    </span>
  {/if}
{/snippet}
