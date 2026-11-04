<script lang="ts">
  import FileUp from '@lucide/svelte/icons/file-up';
  import Pencil from '@lucide/svelte/icons/pencil';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import FactsCard from '$lib/components/FactsCard.svelte';
  import { ICON } from '$lib/components/icon';
  import IconButton from '$lib/components/IconButton.svelte';
  import type { ScopeOption } from '$lib/components/scope/scope-options';
  import SectionCard from '$lib/components/SectionCard.svelte';
  import SidePanel from '$lib/components/SidePanel.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { idKey } from '$lib/ipc/knowledge-id';
  import type { KnowledgeDetail } from '$lib/ipc/bindings/KnowledgeDetail';
  import type { KnowledgeDocument } from '$lib/ipc/bindings/KnowledgeDocument';
  import type { KnowledgeLink } from '$lib/ipc/bindings/KnowledgeLink';
  import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
  import { knowledgeReplace } from '$lib/stores/knowledgeReplace.svelte';
  import { knowledgeRemote } from '$lib/stores/knowledgeRemote.svelte';
  import { knowledgeReview } from '$lib/stores/knowledgeReview.svelte';
  import { knowledgeStats } from '$lib/stores/knowledgeStats.svelte';
  import { knowledgeStatus } from '$lib/stores/knowledgeStatus.svelte';
  import { knowledgeViewer } from '$lib/stores/knowledgeViewer.svelte';
  import type { ViewStatus } from '$lib/stores/view-loader.svelte';
  import { readFailureText } from '$lib/utils/knowledge-failure';

  import BodyState from '../memory/BodyState.svelte';
  import { settled } from '../memory/memory-data.svelte';
  import { ChunkTexts } from './chunk-texts.svelte';
  import DocumentActions from './DocumentActions.svelte';
  import DocumentChanges from './DocumentChanges.svelte';
  import DocumentHistory from './DocumentHistory.svelte';
  import DocumentMatches from './DocumentMatches.svelte';
  import DocumentReview from './DocumentReview.svelte';
  import DocumentSections from './DocumentSections.svelte';
  import DocumentTitle from './DocumentTitle.svelte';
  import DropVeil from './DropVeil.svelte';
  import { docFacts, docSubtitle, outlineOf } from './knowledge-format';
  import { inTeamDatabase } from './knowledge-repos';
  import { lookOf } from './knowledge-status';
  import LinksEditor from './LinksEditor.svelte';
  import { joinHistory } from './history-entries';
  import { useReveal } from './panel-reveal.svelte';
  import PanelSpinner from './PanelSpinner.svelte';
  import { decisionPin, liveIdOf, mineOf, reviewModel, type ReviewStep, versionOf } from './review-model';

  interface Props {
    open: boolean;
    /** The document as the list or its own read gave it. Null while one that is not on this page is being read. */
    doc: KnowledgeDocument | null;
    /** The document with its outline; null until it was read. */
    detail: KnowledgeDetail | null;
    options: ScopeOption[];
    /** How the read of the outline stands, and a way to read it again. */
    status: ViewStatus;
    error: string | null;
    onretry: () => void;
    onrename: (title: string) => Promise<boolean>;
    /** Saves the links and answers them as stored, and says so in a toast. Null (with a toast) when the server refused. */
    onlinks: (links: KnowledgeLink[]) => Promise<KnowledgeLink[] | null>;
    ondelete: () => void;
    /** Asks for a file to take the document's place. */
    onreplace: () => void;
    /** A file was dropped on the panel to replace the document. */
    onreplacedrop: () => void;
    /** The user may not change the document (a read-only role on the remote place): Replace and Delete are off, with the reason under them. */
    readOnly: boolean;
    /** The user chose a step of the review. `text` is the comment of a request for changes, or the note that goes with a new file. */
    onstep: (step: ReviewStep, text: string) => void;
    /** Saves the original file of the document, or of its live version. */
    ondownload: (live: boolean) => void;
    /** Opens the full review of the pending changes, at a section when one was chosen. */
    onreview: (section: string | null) => void;
    onclose: () => void;
    /** The search of the documents table as typed: while it holds words, the panel shows where they are in the document. */
    query: string;
  }

  let { open, doc, detail, status, error, options, query, onretry, onrename, onlinks, ondelete, onreplace, onreplacedrop, readOnly, onstep, ondownload, onreview, onclose }: Props = $props();

  const searched = settled(() => query);
  const searching = $derived(searched.value.trim() !== '');

  /** The texts of this document's chunks, read as the user opens them. */
  const docId = $derived(doc?.id ?? null);
  const texts = $derived(docId === null ? null : new ChunkTexts(docId));

  // Drafts, approval and the review exist for a Harness Knowledge that keeps them (`docs-drafts`): without it the panel is the one it always was.
  const drafts = $derived(knowledgeStatus.value?.features.docsDrafts === true);
  /** A document in the team database can only link to git projects: teammates could not find a folder without a git remote. */
  const inTeam = $derived(doc !== null && inTeamDatabase(doc, knowledgeRemote.value));
  const version = $derived(drafts && doc ? versionOf(doc) : null);
  const look = $derived(version && doc ? lookOf(doc) : null);
  const approval = $derived(knowledgeChanges.pulse?.approval ?? knowledgeStats.value?.approval ?? null);
  /** The events of what the History section is about (the pending version of a live document that has one): only once they were read for it, never those of the document shown before. */
  const ownEvents = $derived(doc ? knowledgeReview.eventsOf(version?.id ?? doc.id) : null);
  const versionEvents = $derived(version ? ownEvents : null);
  /** The diff on screen: a verdict is about it, so it must be the version's latest text (see `decisionPin`). */
  const compare = $derived(version ? knowledgeReview.shownCompare(version.id) : null);
  const model = $derived(
    version
      ? reviewModel({
          version,
          approval,
          mine: mineOf(version, versionEvents ?? [], (name) => knowledgeViewer.is(name)),
          readOnly,
          noName: knowledgeViewer.noName,
          pin: decisionPin(compare, version).reason,
          events: versionEvents ?? [],
          lastChange: doc?.lastChange ?? null,
        })
      : null,
  );

  const replaceInReview = $derived(model?.actions.some((action) => action.step === 'replace') ?? false);

  // The History section is the whole story of the document: of a live document with a pending update, the events of both rows in one timeline.
  const liveId = $derived(doc && version ? liveIdOf(doc) : null);
  const liveEvents = $derived(liveId === null ? null : knowledgeReview.liveEventsOf(liveId));
  const timeline = $derived(liveId === null ? ownEvents : ownEvents && liveEvents ? joinHistory(ownEvents, liveEvents) : null);
  const loaders = $derived(liveId === null ? [knowledgeReview.history] : [knowledgeReview.history, knowledgeReview.liveHistory]);
  const failed = $derived(loaders.find((loader) => loader.error !== null)?.error ?? null);
  const historyStatus = $derived<ViewStatus>(failed ? 'error' : loaders.some((loader) => loader.status === 'loading') ? 'loading' : knowledgeReview.history.status);
  const stepping = $derived(version !== null && knowledgeReview.busy.has(String(version.id)));

  // The body waits until every read of the document is in (its sections, and with drafts its history and its diff, or their failures), behind one
  // spinner: shown part by part, the sections would push each other up and down while they arrive.
  const compareSettled = $derived(version === null || compare !== null || knowledgeReview.compareOutcome(version.id) !== null || knowledgeReview.compareFailed(version.id));
  const historySettled = $derived(!drafts || timeline !== null || failed !== null);
  const reveal = useReveal(() => (open && doc ? idKey(doc.id) : null), () => (detail !== null || status === 'error') && historySettled && compareSettled);

  let renaming = $state(false);
  /** The links as the user left them while the server has not answered yet. A save says so in a toast (see `knowledgeDocs.setLinks`). */
  let draft = $state.raw<KnowledgeLink[] | null>(null);
  let sent = 0;

  // Another document starts from what it has, with no field open.
  $effect(() => {
    void doc?.id;

    renaming = false;
    draft = null;
  });

  // A file dragged over the panel replaces its document, unless the document may not be changed. The page's own drop target steps aside while it is over the panel.
  const droppable = $derived(doc !== null && !readOnly && !knowledgeReplace.busy && !knowledgeReplace.picking);

  const carriesFiles = (event: DragEvent): boolean => droppable && (event.dataTransfer?.types.includes('Files') ?? false);

  function dragEnter(event: DragEvent): void {
    if (carriesFiles(event)) {
      event.preventDefault();
      knowledgeReplace.overPanel = true;
    }
  }

  function dragOver(event: DragEvent): void {
    if (carriesFiles(event)) {
      event.preventDefault();
    }
  }

  function dragLeave(event: DragEvent): void {
    const inside = event.relatedTarget instanceof Node && event.currentTarget instanceof Node && event.currentTarget.contains(event.relatedTarget);

    if (!inside) {
      knowledgeReplace.overPanel = false;
    }
  }

  function drop(event: DragEvent): void {
    if (carriesFiles(event)) {
      event.preventDefault();
      knowledgeReplace.overPanel = false;
      onreplacedrop();
    }
  }

  // The shell tells where a file dragged in from the file manager is, with no element to go by: the page asks the panel for its place.
  let element = $state<HTMLElement | null>(null);

  $effect(() => {
    const panel = element;

    if (!droppable || panel === null) {
      return;
    }

    knowledgeReplace.target = { rect: () => panel.getBoundingClientRect(), drop: () => onreplacedrop() };

    return () => {
      knowledgeReplace.target = null;
    };
  });

  // A panel that goes away (another document, closing) with a drag over it must not leave the page's drop target hidden.
  $effect(() => () => {
    knowledgeReplace.overPanel = false;
  });

  async function change(next: KnowledgeLink[]): Promise<void> {
    sent += 1;
    const mine = sent;

    draft = next;

    await onlinks(next);

    // A newer change owns what is on screen.
    if (mine !== sent) {
      return;
    }

    draft = null;
  }
</script>

{#snippet rename()}
  <IconButton icon={Pencil} iconSize={ICON.sm} label={t('knowledge.panel.rename')} onclick={() => (renaming = true)} />
{/snippet}

{#snippet dropVeil()}
  <DropVeil shown={knowledgeReplace.overPanel && droppable} icon={FileUp} title={t('knowledge.replace.drop.title')} hint={t('knowledge.replace.drop.hint', { title: doc?.title ?? '' })} />
{/snippet}

{#snippet field()}
  {#if doc}
    <DocumentTitle title={doc.title} onsave={onrename} onleave={() => (renaming = false)} />
  {/if}
{/snippet}

{#snippet statusRow()}
  {#if doc && look}
    <!-- The state is said once, in the header under the title, as a badge of the size the table's Format badge has, with what the line under the title says in grey. -->
    <Badge tone={look.tone} outline={look.outline} size="lg" class="shrink-0"><look.icon size={ICON.sm} aria-hidden="true" />{look.label}</Badge>
    <span class="min-w-0 truncate text-xs text-fg-3" title={docSubtitle(doc)}>{docSubtitle(doc)}</span>
  {/if}
{/snippet}

<SidePanel
  {open}
  title={doc?.title ?? ''}
  subtitle={doc && !look ? docSubtitle(doc) : ''}
  status={look ? statusRow : undefined}
  actions={doc ? rename : undefined}
  titleField={renaming ? field : undefined}
  overlay={dropVeil}
  bind:element
  ondragenter={dragEnter}
  ondragover={dragOver}
  ondragleave={dragLeave}
  ondrop={drop}
  {onclose}
>
  {#if doc && !reveal.shown}
    <PanelSpinner label={t('knowledge.panel.loading')} />
  {:else if doc}
    {#if doc.warnings.length > 0}
      <ul class="flex flex-col gap-1 rounded-md bg-warning-wash px-3 py-2 text-sm text-warning" role="status">
        {#each doc.warnings as warning (warning)}
          <li>{warning}</li>
        {/each}
      </ul>
    {/if}

    {#if searching && texts}
      <SectionCard flat heading={t('knowledge.panel.matches')} hint={t('knowledge.matches.hint')}>
        <DocumentMatches docId={doc.id} query={searched.value} {texts} />
      </SectionCard>
    {/if}

    {#if model && version}
      <SectionCard flat heading={t('knowledge.drafts.panel.review')} hint={t('knowledge.drafts.panel.review.hint')}>
        <DocumentReview {model} busy={stepping} {onstep} />
      </SectionCard>

      <SectionCard flat heading={t('knowledge.drafts.panel.changes')} hint={t('knowledge.drafts.panel.changes.hint')}>
        <DocumentChanges {compare} status={knowledgeReview.compare.status} {onreview} />
      </SectionCard>
    {/if}

    <!-- Every section says what it is under its heading. Without links, the hint says what that means. -->
    <SectionCard flat heading={t('knowledge.panel.links')} hint={t((draft ?? doc.links).length === 0 ? 'knowledge.links.global' : 'knowledge.panel.links.hint')}>
      <LinksEditor links={draft ?? doc.links} onchange={(next) => void change(next)} {options} team={inTeam} {readOnly} />
    </SectionCard>

    <SectionCard flat heading={t('knowledge.panel.sections')} hint={t('knowledge.sections.hint')}>
      <BodyState loaded={detail !== null} {status} {error} failed={t('knowledge.panel.sections.failed')} loading={t('knowledge.panel.sections.loading')} {onretry}>
        {#if texts}<DocumentSections sections={detail?.sections ?? []} {texts} />{/if}
      </BodyState>
    </SectionCard>

    <FactsCard flat heading={t('knowledge.panel.details')} hint={t('knowledge.panel.details.hint')} rows={docFacts(doc, detail ? outlineOf(detail.sections).length : null)} />

    <SectionCard flat heading={t('knowledge.panel.actions')} hint={t('knowledge.panel.actions.hint')}>
      <!-- The Review section offers Replace file… itself while changes are requested (it asks for approval again in the same step), so Actions leaves it out then. -->
      <DocumentActions update={doc.pending != null} {readOnly} replace={!replaceInReview} {ondownload} {onreplace} {ondelete} />
    </SectionCard>

    {#if drafts}
      <SectionCard flat heading={t('knowledge.drafts.panel.history')} hint={t('knowledge.drafts.panel.history.hint')}>
        <DocumentHistory events={timeline} status={historyStatus} error={failed ? readFailureText(failed) : null} onretry={() => void Promise.all(loaders.map((loader) => loader.refresh()))} />
      </SectionCard>
    {/if}
  {:else if open && status === 'error'}
    <!-- A document that is not on this page and cannot be read (its place is unreachable) has nothing else to show. -->
    <div class="flex flex-col items-start gap-3">
      <p class="text-sm text-danger" role="alert">{error ?? t('knowledge.panel.sections.failed')}</p>
      <Button onclick={onretry}>{t('memory.tryAgain')}</Button>
    </div>
  {/if}
</SidePanel>
