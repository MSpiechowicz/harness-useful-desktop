<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import DiffView from '$lib/components/DiffView.svelte';
  import PaneHeader from '$lib/components/PaneHeader.svelte';
  import { anchorIndex, type RangeTarget } from '$lib/git/comment-range';
  import { flatLines } from '$lib/git/diff-rows';
  import { statusName, statusTone } from '$lib/git/entries';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeComment } from '$lib/ipc/bindings/KnowledgeComment';
  import type { KnowledgeSectionDiff } from '$lib/ipc/bindings/KnowledgeSectionDiff';
  import { idKey, type KnowledgeId } from '$lib/ipc/knowledge-id';
  import { type DraftLineComment, MAX_COMMENT_BYTES } from '$lib/stores/knowledgeReviewDraft.svelte';

  import { diffMode } from '../changes/diff-mode.svelte';
  import { savedSectionRef, sectionKey, sectionLabel } from './review-changes';
  import ReviewThread from './ReviewThread.svelte';
  import ReviewThreads from './ReviewThreads.svelte';

  interface Props {
    /** The section on show. */
    diff: KnowledgeSectionDiff;
    /** The comments being written on this section, kept until the review is sent. */
    drafts: DraftLineComment[];
    /** Every saved comment of the document. */
    saved: readonly KnowledgeComment[];
    /** The keys (`sectionKey`) of the sections these changes list. A comment on any other section (an unchanged one, or one the text no longer has) is shown under every section. */
    known: readonly string[];
    /** Whether lines can be commented on: not in the comparison with an earlier version, whose lines are not the ones the server numbers. */
    commentable: boolean;
    /** What says why lines cannot be commented on, or how to comment. */
    hint: string;
    readOnly: boolean;
    busy: boolean;
    oncomment: (target: RangeTarget, body: string) => void;
    onedit: (id: number, body: string) => void;
    onremove: (id: number) => void;
    onreply: (commentId: KnowledgeId, body: string) => Promise<boolean>;
    onresolve: (commentId: KnowledgeId, resolved: boolean) => void;
    ondelete: (commentId: KnowledgeId) => Promise<boolean>;
  }

  let { diff, drafts, saved, known, commentable, hint, readOnly, busy, oncomment, onedit, onremove, onreply, onresolve, ondelete }: Props = $props();

  const roots = $derived(saved.filter((comment) => comment.parentId == null));
  const overall = $derived(roots.filter((comment) => comment.section == null));
  const onSection = $derived(
    roots.filter((comment) => {
      const ref = savedSectionRef(comment);

      return ref !== null && ref.section === diff.path && ref.occurrence === diff.occurrence;
    }),
  );

  const lines = $derived(flatLines(diff));

  /** The end line of a comment on lines, or null for one that is not on lines. */
  const endOf = (comment: KnowledgeComment): number | null => (comment.side != null && comment.from != null ? (comment.to ?? comment.from) : null);

  // A thread sits under its line when this diff shows that line. The comparison with an earlier version numbers its lines differently, so there none does.
  const placed = $derived(commentable ? onSection.filter((comment) => comment.side != null && endOf(comment) !== null && anchorIndex(lines, comment.side, endOf(comment) as number) !== -1) : []);
  const anchors = $derived(placed.map((comment) => ({ key: idKey(comment.id), side: comment.side as NonNullable<KnowledgeComment['side']>, endLine: endOf(comment) as number })));
  const byKey = $derived(new Map(placed.map((comment) => [idKey(comment.id), comment])));

  /** The threads of sections these changes do not list: no section of the list would ever show them otherwise. */
  const offList = $derived(
    roots.filter((comment) => {
      const ref = savedSectionRef(comment);

      return ref !== null && !known.includes(sectionKey(ref.section, ref.occurrence));
    }),
  );

  /** What no line shows here: its lines are gone or moved, the comparison hides them, or its section is not in the list. */
  const elsewhere = $derived([...onSection.filter((comment) => !byKey.has(idKey(comment.id))), ...offList]);
</script>

{#snippet savedThread(key: string)}
  {@const root = byKey.get(key)}
  {#if root}<ReviewThread {root} all={saved} {readOnly} {busy} {onreply} {onresolve} {ondelete} />{/if}
{/snippet}

{#snippet meta()}
  <span class="flex shrink-0"><Badge tone={statusTone(diff.status)}>{statusName(diff.status)}</Badge></span>
  <span class="tabular shrink-0 text-xs whitespace-nowrap"><span class="text-success">+{diff.additions}</span> <span class="text-danger">−{diff.deletions}</span></span>
{/snippet}

{#snippet actions()}
  <p class="hidden min-w-0 truncate text-xs text-fg-3 @2xl:block">{hint}</p>
{/snippet}

<div class="flex size-full min-h-0 flex-col">
  <PaneHeader title={sectionLabel(diff.path, diff.occurrence)} label={diff.path} {meta} {actions} />

  <div class="min-h-0 flex-1">
    {#key sectionKey(diff.path, diff.occurrence)}
      <DiffView {diff} mode={diffMode.value} comments={commentable ? drafts : []} saved={anchors} {savedThread} maxBytes={MAX_COMMENT_BYTES} placeholder={t('knowledge.drafts.review.lineCommentPlaceholder')} {oncomment} {onedit} {onremove} />
    {/key}
  </div>

  {#if overall.length > 0 || elsewhere.length > 0}
    <!-- Only what no line can hold: the overall comments of the reviews, and the threads whose lines this diff does not show. The others sit under their lines. -->
    <section class="max-h-[40%] shrink-0 overflow-y-auto border-t border-panel-edge page-gutter py-4" aria-label={t('knowledge.drafts.thread.label')}>
      {#if overall.length > 0}
        <!-- Headed like a section of a panel: the small caps heading, then the grey line that says what the list is. -->
        <h3 class="mb-1 text-xs font-medium tracking-wider text-fg-3 uppercase">{tn('knowledge.drafts.thread.overall', overall.length)}</h3>
        <p class="mb-3.5 text-sm text-fg-3">{t('knowledge.drafts.thread.overall.hint')}</p>
        <ReviewThreads roots={overall} all={saved} {readOnly} {busy} {onreply} {onresolve} {ondelete} />
      {/if}

      {#if elsewhere.length > 0}
        <h3 class="mb-1 text-xs font-medium tracking-wider text-fg-3 uppercase {overall.length > 0 ? 'mt-5' : ''}">{tn('knowledge.drafts.thread.unplaced', elsewhere.length)}</h3>
        <p class="mb-3.5 text-sm text-fg-3">{t('knowledge.drafts.thread.unplaced.hint')}</p>
        <ReviewThreads roots={elsewhere} named all={saved} {readOnly} {busy} {onreply} {onresolve} {ondelete} />
      {/if}
    </section>
  {/if}
</div>
