<script lang="ts">
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import Check from '@lucide/svelte/icons/check';
  import Columns2 from '@lucide/svelte/icons/columns-2';
  import GitPullRequest from '@lucide/svelte/icons/git-pull-request';
  import History from '@lucide/svelte/icons/history';
  import Lock from '@lucide/svelte/icons/lock';
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import MessageSquareWarning from '@lucide/svelte/icons/message-square-warning';
  import Rows2 from '@lucide/svelte/icons/rows-2';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import ChangeList from '$lib/components/ChangeList.svelte';
  import type { Choice, ReviewLineComment } from '$lib/components/change-list';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import FinishReviewDialog, { type FinishLabels } from '$lib/components/FinishReviewDialog.svelte';
  import { ICON } from '$lib/components/icon';
  import IconButton from '$lib/components/IconButton.svelte';
  import LoadingSpinner from '$lib/components/LoadingSpinner.svelte';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import WorkspaceLayout from '$lib/components/WorkspaceLayout.svelte';
  import WorkspaceToggles from '$lib/components/WorkspaceToggles.svelte';
  import { WorkspaceState } from '$lib/components/workspace.svelte';
  import type { RangeTarget } from '$lib/git/comment-range';
  import { type MessageKey, t, tn } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeVerdict } from '$lib/ipc/bindings/KnowledgeVerdict';
  import { idKey, type KnowledgeId } from '$lib/ipc/knowledge-id';
  import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
  import { knowledgeDocs } from '$lib/stores/knowledgeDocs.svelte';
  import { knowledgeRemote } from '$lib/stores/knowledgeRemote.svelte';
  import { knowledgeReview } from '$lib/stores/knowledgeReview.svelte';
  import { knowledgeReviewDraft, MAX_COMMENT_BYTES, MAX_LINE_COMMENTS } from '$lib/stores/knowledgeReviewDraft.svelte';
  import { knowledgeStats } from '$lib/stores/knowledgeStats.svelte';
  import { knowledgeViewer } from '$lib/stores/knowledgeViewer.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';

  import { diffMode, type DiffMode } from '../changes/diff-mode.svelte';
  import { keptRemote } from './knowledge-replace';
  import GrowField from './GrowField.svelte';
  import { changeSections, commentCounts, sectionCommentBlock, sectionKey, sectionKeys, sectionLabel, sinceRevisionOf, totalsOf } from './review-changes';
  import { decisionPin, mineOf, versionOf } from './review-model';
  import ReviewMain from './ReviewMain.svelte';

  interface Props {
    /** The document whose pending changes are reviewed. */
    id: KnowledgeId;
    /** Leaves the review, back to the documents. */
    onclose: () => void;
  }

  let { id, onclose }: Props = $props();

  const workspace = new WorkspaceState('changes', { details: false });

  const doc = $derived(knowledgeDocs.detail.data?.document.id === id ? knowledgeDocs.detail.data.document : null);
  const version = $derived(doc ? versionOf(doc) : null);
  const approval = $derived(knowledgeChanges.pulse?.approval ?? knowledgeStats.value?.approval ?? null);
  // A read-only role on the remote place cannot answer or resolve comments, nor send a review.
  const readOnly = $derived(doc !== null && keptRemote(doc, knowledgeRemote.value) && knowledgeStats.remoteWritable === false);

  // The whole change, or only what changed since the version the reviewer last saw.
  let sinceOn = $state(false);
  // Only the comments read for this version: the loader keeps the last document's while the next one loads.
  const saved = $derived((version ? knowledgeReview.commentsOf(version.id) : null) ?? []);
  // Only an earlier version can be compared with: one the reviewer commented on while it was at a lower revision than now.
  const reviewed = $derived(version ? sinceRevisionOf(saved, version.author, knowledgeViewer.name) : null);
  const sinceRevision = $derived(version && reviewed !== null && reviewed < version.revision ? reviewed : null);

  $effect(() => {
    const key = sinceOn && version && sinceRevision !== null ? `${idKey(version.id)}|${sinceRevision}` : null;

    knowledgeReview.since.sync(key, null);
  });

  // The diff on screen is the one a verdict is about: it is read for this version, and a verdict waits until it is the version's latest text.
  const wholeOutcome = $derived(version ? knowledgeReview.compareOutcome(version.id) : null);
  const whole = $derived(version ? knowledgeReview.shownCompare(version.id) : null);
  const sinceOutcome = $derived(sinceOn && version && sinceRevision !== null ? knowledgeReview.sinceOutcome(version.id, sinceRevision) : null);
  const sinceCompare = $derived(sinceOutcome?.type === 'done' ? sinceOutcome.value : null);
  // A verdict is about the text the reviewer reads: the diff since an earlier version stands as well as the whole change, each as of the row and revision it was read for.
  const pin = $derived.by(() => {
    if (!version) {
      return null;
    }

    const wholePin = decisionPin(whole, version);

    return wholePin.reason === null && sinceOn && sinceCompare ? decisionPin(sinceCompare, version) : wholePin;
  });
  /** The revision of the text the reviewer reads, which the line comments are written on. */
  const shownRevision = $derived(whole?.revision ?? version?.revision ?? null);
  /** The comparison with the version the reviewer last saw was not kept: the whole change shows, and the toggle says so. */
  const sinceLost = $derived(sinceOn && sinceOutcome?.type === 'refused');
  const shown = $derived(sinceOn && sinceCompare ? sinceCompare : whole);
  const sections = $derived(shown?.sections ?? []);
  const ids = $derived(sectionKeys(sections));
  const totals = $derived(totalsOf(whole?.sections ?? []));

  let selected = $state<string | null>(null);

  // The section the summary named, else the first; a section that is gone (the list changed) gives way to the first.
  $effect(() => {
    if (selected !== null && ids.includes(selected)) {
      return;
    }

    const wanted = viewState.knowledgeReviewSection;
    // The summary names a section by its key: the heading, with a number for a heading that repeats.
    const named = wanted === null ? -1 : ids.indexOf(wanted);

    selected = ids[Math.max(0, named)] ?? null;
  });

  const index = $derived(selected === null ? -1 : ids.indexOf(selected));
  const diff = $derived(index === -1 ? null : sections[index]);

  const drafts = $derived(knowledgeReviewDraft.comments(id));
  const counts = $derived(commentCounts(saved, drafts));
  const listed = $derived(changeSections(sections, counts));

  // The user owns a version they wrote or asked a review of: the server refuses them to approve or ask for changes on it.
  const events = $derived(version ? knowledgeReview.eventsOf(version.id) : null);
  const mine = $derived(version ? mineOf(version, events ?? [], (name) => knowledgeViewer.is(name)) : null);
  // The author of a version only comments: approving and asking for changes are for teammates, and only while it waits for approval.
  const decides = $derived(version !== null && version.status === 'waiting' && approval === true && mine !== true);

  const modes = $derived<StripOption<DiffMode>[]>([
    { value: 'unified', label: t('changes.header.unified'), icon: Rows2 },
    { value: 'split', label: t('changes.header.split'), icon: Columns2 },
  ]);

  const scopes = $derived<StripOption<'whole' | 'since'>[]>([
    { value: 'whole', label: t('knowledge.drafts.review.whole') },
    { value: 'since', label: t(knowledgeViewer.name === null ? 'knowledge.drafts.review.sinceLast' : 'knowledge.drafts.review.since'), disabled: sinceRevision === null, reason: sinceRevision === null ? t('knowledge.drafts.review.sinceNone') : undefined },
  ]);

  /** Why the section on show takes no comment (it is beyond what the server keeps one on), or null. */
  const sectionBlock = $derived(diff ? sectionCommentBlock(diff) : null);

  function comment(target: RangeTarget, body: string): void {
    if (diff && sectionBlock === null && shownRevision !== null && !knowledgeReviewDraft.add(id, shownRevision, { section: diff.path, occurrence: diff.occurrence }, target, body)) {
      toasts.push({ title: t('changes.toast.limit.title'), description: t('changes.toast.limit.description', { max: MAX_LINE_COMMENTS }), tone: 'warning' });
    }
  }

  /** The line comments belong to the text they were written on: a review that was drafted on an earlier revision is sent as that revision, so the server says the document changed. */
  const stale = $derived(shownRevision !== null && knowledgeReviewDraft.stale(id, shownRevision));
  const target = $derived(version && doc ? { id: version.id, revision: knowledgeReviewDraft.revision(id) ?? shownRevision ?? version.revision, title: doc.title, author: version.author } : null);

  /** The user looked at the new version and keeps their comments: they are about it from now on. */
  function keep(): void {
    if (shownRevision !== null) {
      knowledgeReviewDraft.rebase(id, shownRevision);
    }
  }

  /** Why the review cannot be finished: the document is still being read, or there is nothing to review. */
  const finishBlock = $derived(target !== null ? null : doc === null ? t('knowledge.drafts.review.finishLoading') : t('knowledge.drafts.review.nothing'));

  async function reply(commentId: KnowledgeId, body: string): Promise<boolean> {
    return target ? knowledgeReview.reply(target, commentId, body) : false;
  }

  async function deleteComment(commentId: KnowledgeId): Promise<boolean> {
    return target ? knowledgeReview.remove(target, commentId) : false;
  }

  function resolve(commentId: KnowledgeId, resolved: boolean): void {
    if (target) {
      void knowledgeReview.resolve(target, commentId, resolved);
    }
  }

  // ----- Finish review
  let finishing = $state(false);
  const note = $derived(knowledgeReviewDraft.note(id));
  const verdict = $derived(decides ? knowledgeReviewDraft.verdict(id) : 'comment');

  const verdicts = $derived<Choice[] | undefined>(
    decides
      ? [
          { value: 'comment', title: t('knowledge.drafts.verdict.comment'), detail: t('knowledge.drafts.verdict.comment.detail'), icon: MessageSquare },
          { value: 'approve', title: t('knowledge.drafts.verdict.approve'), detail: t('knowledge.drafts.verdict.approve.detail'), icon: Check },
          { value: 'requestChanges', title: t('knowledge.drafts.verdict.requestChanges'), detail: t('knowledge.drafts.verdict.requestChanges.detail'), icon: MessageSquareWarning },
        ]
      : undefined,
  );

  const finishComments = $derived<ReviewLineComment[]>(drafts.map((draft) => ({ id: draft.id, path: sectionLabel(draft.section, draft.occurrence), side: draft.side, startLine: draft.startLine, endLine: draft.endLine, body: draft.body })));

  /** Why the verdict waits: approving and asking for changes are about the diff on screen, which must be the version's latest text. A comment is not stopped. */
  const verdictBlock = $derived(verdict !== 'comment' ? (pin?.reason ?? null) : null);

  /** What each verdict needs: approving nothing more, a comment needs words (an overall one or on lines), a request for changes needs its overall comment. */
  const canSubmit = $derived.by(() => {
    if (verdictBlock !== null) {
      return false;
    }

    if (verdict === 'approve') {
      return true;
    }

    return verdict === 'requestChanges' ? note.trim() !== '' : note.trim() !== '' || drafts.length > 0;
  });

  /** The words of the dialog for a document: sections where a code review has files, a comment where it has an instruction. */
  const finishLabels = $derived<FinishLabels>({
    title: t('knowledge.drafts.finish.title'),
    found: (count, groups) => tn('knowledge.drafts.finish.found', count, { groups: tn('knowledge.drafts.finish.inSections', groups) }),
    search: t('knowledge.drafts.finish.search'),
    groups: t('knowledge.drafts.finish.groups'),
    tooLong: (kib) => t('knowledge.drafts.finish.tooLong', { kib }),
  });

  /** Whether the overall comment is needed: a request for changes needs it, and so does a comment that has no line comments to stand on. */
  const noteNeed = $derived(verdict === 'approve' ? t('knowledge.drafts.optional') : verdict === 'requestChanges' ? t('knowledge.drafts.required') : drafts.length > 0 ? t('knowledge.drafts.optional') : t('knowledge.drafts.finish.requiredUnlessLines'));

  const SUBMIT_LABELS = { comment: 'knowledge.drafts.verdict.comment.submit', approve: 'knowledge.drafts.verdict.approve.submit', requestChanges: 'knowledge.drafts.verdict.requestChanges.submit' } as const satisfies Record<KnowledgeVerdict, MessageKey>;

  const submitLabel = $derived(t(SUBMIT_LABELS[verdict]));

  async function submit(): Promise<void> {
    if (!target || !canSubmit) {
      return;
    }

    const asked: KnowledgeVerdict = verdict;
    const result = await knowledgeReview.submit(target, { verdict: asked, body: note.trim() || null, comments: knowledgeReviewDraft.lines(id) }, { label: t('knowledge.drafts.review.staleKeep'), run: keep });

    if (result === null) {
      return;
    }

    knowledgeReviewDraft.clear(id);
    finishing = false;

    // Approving or asking for changes ends the review of this version.
    if (asked !== 'comment') {
      onclose();
    }
  }

  function jump(found: ReviewLineComment): void {
    const draft = drafts.find((each) => each.id === found.id);
    const key = draft ? sectionKey(draft.section, draft.occurrence) : undefined;

    if (key !== undefined && ids.includes(key)) {
      selected = key;
    }

    finishing = false;
  }

  function onkeydown(event: KeyboardEvent): void {
    // Escape goes back, unless something that owns the key is open (a dialog, a comment being written).
    if (event.key === 'Escape' && !event.defaultPrevented && !finishing) {
      event.preventDefault();
      onclose();
    }
  }
</script>

<svelte:window {onkeydown} />

<ViewHeader title={t('knowledge.drafts.review.title')} context={doc ? `${doc.title} · ${tn('knowledge.drafts.changes.inSections', totals.sections)} · +${totals.additions} −${totals.deletions}` : undefined}>
  {#snippet leading()}
    <IconButton icon={ArrowLeft} label={t('knowledge.drafts.review.back')} shortcut="escape" side="bottom" onclick={onclose} />
  {/snippet}

  {#snippet actions()}
    <!-- First, in the amber of the review pill of the Changes view (ReviewButton), so both reviews look and sit alike. -->
    <Tooltip text={finishBlock ?? ''} disabled={finishBlock === null} side="bottom">
      {#snippet children({ describedby })}
        <span aria-describedby={finishBlock ? describedby : undefined} class="inline-flex">
          <button
            type="button"
            aria-haspopup="dialog"
            aria-label={drafts.length > 0 ? t('knowledge.drafts.review.finishCount', { count: drafts.length }) : t('knowledge.drafts.review.finish')}
            disabled={target === null}
            class="group rounded-md focus-visible:outline-2 focus-visible:outline-offset-1 disabled:cursor-not-allowed disabled:opacity-50"
            onclick={() => (finishing = true)}
          >
            <Badge tone="warning" class="h-control rounded-md! px-2.5 text-sm! transition-colors duration-[var(--dur-fast)] group-enabled:group-hover:bg-warning/20">
              <GitPullRequest size={ICON.sm} aria-hidden="true" />
              {t('knowledge.drafts.review.finish')}
              {#if drafts.length > 0}<span class="tabular rounded-full bg-warning/20 px-1.5 text-xs leading-5">{drafts.length}</span>{/if}
            </Badge>
          </button>
        </span>
      {/snippet}
    </Tooltip>

    {#if sinceRevision !== null || sinceOn}
      <SegmentedControl options={scopes} value={sinceOn ? 'since' : 'whole'} label={t('knowledge.drafts.review.scope')} onchange={(next) => (sinceOn = next === 'since')} />
    {/if}

    <SegmentedControl options={modes} value={diffMode.value} label={t('changes.header.layout')} onchange={(mode) => diffMode.set(mode)} />

  {/snippet}

  {#snippet fixed()}
    <WorkspaceToggles {workspace} listLabel={t('knowledge.drafts.changes.sections')} />
  {/snippet}
</ViewHeader>

{#if stale}
  <!-- The line comments were written on an earlier text: they stay, and are said to be about it until the user keeps them on the new one. -->
  <div class="flex shrink-0 flex-nowrap items-center gap-3 border-b border-panel-edge page-gutter py-2" role="status">
    <History size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />
    <p class="min-w-0 flex-1 truncate text-sm text-fg-3">{t('knowledge.drafts.review.stale')}</p>
    <Button onclick={keep}>{t('knowledge.drafts.review.staleKeep')}</Button>
  </div>
{/if}

<div class="relative min-h-0 flex-1">
  {#if shown === null && !(version && knowledgeReview.compareFailed(version.id)) && wholeOutcome?.type !== 'refused'}
    <LoadingSpinner label={t('knowledge.drafts.changes.loading')} />
  {:else if shown === null || sections.length === 0}
    <div class="grid size-full place-items-center">
      <EmptyState icon={GitPullRequest} title={t('knowledge.drafts.review.nothing')} body={t('knowledge.drafts.review.nothing.body')}>
        {#snippet action()}
          <Button onclick={onclose}>{t('knowledge.drafts.review.back')}</Button>
        {/snippet}
      </EmptyState>
    </div>
  {:else}
    <WorkspaceLayout {workspace} listLabel={t('knowledge.drafts.changes.sections')} resizeLabel={t('knowledge.drafts.review.resize')} scroll="own">
      {#snippet list()}
        <div class="relative min-h-0 flex-1 overflow-y-auto">
          <ChangeList
            sections={listed}
            selectedId={selected}
            label={t('knowledge.drafts.changes.sections')}
            testid="section-list"
            locked
            isOpen={() => true}
            ontoggle={() => {}}
            onselect={(next) => (selected = next)}
            onpick={() => workspace.drawer.close()}
          />
        </div>
      {/snippet}

      {#snippet main()}
        {#if diff}
          <ReviewMain
            {diff}
            drafts={drafts.filter((draft) => sectionKey(draft.section, draft.occurrence) === selected)}
            {saved}
            known={ids}
            commentable={!(sinceOn && sinceCompare) && sectionBlock === null}
            hint={sectionBlock ?? (sinceLost ? t('knowledge.drafts.review.sinceLost') : sinceOn && sinceCompare ? t('knowledge.drafts.review.sinceHint') : mine === true ? t('knowledge.drafts.review.hint.author') : t('knowledge.drafts.review.hint'))}
            {readOnly}
            busy={target !== null && knowledgeReview.busy.has(idKey(target.id))}
            oncomment={comment}
            onedit={(commentId, body) => knowledgeReviewDraft.edit(id, commentId, body)}
            onremove={(commentId) => knowledgeReviewDraft.remove(id, commentId)}
            onreply={reply}
            onresolve={resolve}
            ondelete={deleteComment}
          />
        {/if}
      {/snippet}
    </WorkspaceLayout>
  {/if}
</div>

<FinishReviewDialog
  bind:open={finishing}
  comments={finishComments}
  {note}
  maxBytes={MAX_COMMENT_BYTES}
  onnote={(next) => knowledgeReviewDraft.setNote(id, next)}
  onedit={(commentId, body) => knowledgeReviewDraft.edit(id, commentId, body)}
  onremove={(commentId) => knowledgeReviewDraft.remove(id, commentId)}
  onjump={jump}
  {submitLabel}
  submitDisabled={!canSubmit || (target !== null && knowledgeReview.busy.has(idKey(target.id)))}
  onsubmit={() => void submit()}
  labels={finishLabels}
  {verdicts}
  verdictLabel={t('knowledge.drafts.verdict.label')}
  bind:verdict={() => verdict, (next) => knowledgeReviewDraft.setVerdict(id, next as KnowledgeVerdict)}
>
  {#snippet leading()}
    {#if verdictBlock}
      <p class="flex min-w-0 items-start gap-2 text-sm text-fg-3" role="status"><Lock size={ICON.sm} class="mt-0.5 shrink-0" aria-hidden="true" />{verdictBlock}</p>
    {/if}
  {/snippet}

  {#snippet noteField()}
    <GrowField bind:value={() => note, (next) => knowledgeReviewDraft.setNote(id, next)} label={t('knowledge.drafts.finish.noteLabel')} note={noteNeed} placeholder={t('knowledge.drafts.finish.notePlaceholder')} />
  {/snippet}
</FinishReviewDialog>
