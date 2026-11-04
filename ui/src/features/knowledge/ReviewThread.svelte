<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import Lock from '@lucide/svelte/icons/lock';
  import Reply from '@lucide/svelte/icons/reply';
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import Trash2 from '@lucide/svelte/icons/trash-2';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import IconButton from '$lib/components/IconButton.svelte';
  import { ICON } from '$lib/components/icon';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import { commentHeading } from '$lib/git/comment-range';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeComment } from '$lib/ipc/bindings/KnowledgeComment';
  import { idKey, type KnowledgeId } from '$lib/ipc/knowledge-id';
  import { knowledgeViewer } from '$lib/stores/knowledgeViewer.svelte';
  import { cx } from '$lib/utils/cx';
  import { formatDateTime, relativeTime } from '$lib/utils/format';
  import { uid } from '$lib/utils/ids';

  import { COMMENT_TEXT } from './comment-text';
  import GrowField from './GrowField.svelte';
  import QuoteBlock from './QuoteBlock.svelte';
  import { sectionLabel } from './review-changes';
  import type { ThreadProps } from './thread-props';

  interface Props extends ThreadProps {
    /** The comment that starts the thread. */
    root: KnowledgeComment;
    /** The badge names the section before the lines, for a thread shown away from its section. */
    named?: boolean;
  }

  let { root, named = false, all, readOnly, busy, onreply, onresolve, ondelete }: Props = $props();

  const readOnlyId = uid('thread-read-only');

  let replying = $state(false);
  let text = $state('');
  /** The comment waiting for an answer to "are you sure?". */
  let deleting = $state<KnowledgeComment | null>(null);
  let confirmOpen = $state(false);

  const resolved = $derived(root.resolvedAt != null);
  const replies = $derived(all.filter((comment) => comment.parentId != null && idKey(comment.parentId) === idKey(root.id)));
  const range = $derived(root.section != null && root.side != null && root.from != null ? commentHeading({ side: root.side, startLine: root.from, endLine: root.to ?? root.from }) : null);
  // An overall comment of a review says so in the badge where a line comment names its lines.
  const lines = $derived(root.section == null ? (root.parentId == null ? t('knowledge.drafts.thread.whole') : null) : named ? [sectionLabel(root.section, root.occurrence ?? 1), range].filter(Boolean).join(' · ') : range);

  async function send(): Promise<void> {
    if (text.trim() !== '' && (await onreply(root.id, text.trim()))) {
      replying = false;
      text = '';
    }
  }

  /** Only the one who wrote a comment may delete it (the server checks it too). Nothing is offered while the user's name is not known. */
  const mine = (comment: KnowledgeComment): boolean => knowledgeViewer.is(comment.by) === true;

  const deletingReplies = $derived(deleting !== null && deleting.parentId == null ? replies.length : 0);

  function ask(comment: KnowledgeComment): void {
    deleting = comment;
    confirmOpen = true;
  }

  function confirmDelete(): Promise<boolean> | undefined {
    return deleting ? ondelete(deleting.id) : undefined;
  }

  /** The start of a comment, so a screen reader tells the delete buttons of the replies apart. */
  function excerpt(body: string): string {
    const line = body.trim().split('\n')[0];

    return line.length > 40 ? `${line.slice(0, 40)}…` : line;
  }

  function who(name: string | null | undefined): string {
    if (!name) {
      return t('knowledge.drafts.someone');
    }

    return knowledgeViewer.is(name) === true ? t('knowledge.drafts.you') : name;
  }
</script>

{#snippet byline(comment: KnowledgeComment)}
  <!-- The name and the time read as one line, like the chat bubbles. A long name gives way (truncates) before the badges beside it do. -->
  <p class="flex min-h-5 min-w-0 flex-1 items-baseline gap-1.5 text-sm text-fg-3">
    <span class="truncate font-medium text-fg">{who(comment.by)}</span>
    <span aria-hidden="true">·</span>
    <time datetime={comment.at} title={formatDateTime(comment.at)} class="tabular shrink-0 text-xs">{relativeTime(comment.at)}</time>
  </p>
{/snippet}

{#snippet written(comment: KnowledgeComment)}
  <div class="flex min-w-0 items-center gap-2">
    {@render byline(comment)}
    {#if mine(comment)}
      <!-- Taller than the line it sits on, so it overlaps the lines around it instead of making this one taller than the others. -->
      <IconButton icon={Trash2} label={t('knowledge.drafts.thread.deleteReplyOf', { name: who(comment.by), text: excerpt(comment.body) })} iconSize={ICON.sm} class="-my-1 size-7! shrink-0" disabled={readOnly || busy} onclick={() => ask(comment)} />
    {/if}
  </div>
  <p class="mt-1 {COMMENT_TEXT}">{comment.body}</p>
{/snippet}

{#snippet step(label: string, Icon: typeof Reply, run: () => void, variant: 'secondary' | 'dangerQuiet' = 'secondary')}
  <Tooltip text={t('knowledge.panel.readOnly')} disabled={!readOnly} side="top">
    {#snippet children({ describedby })}
      <span aria-describedby={readOnly ? describedby : undefined} class="inline-flex">
        <Button {variant} disabled={readOnly || busy} aria-describedby={readOnly ? readOnlyId : undefined} onclick={run}>
          {#snippet leading()}<Icon size={ICON.sm} aria-hidden="true" />{/snippet}
          {label}
        </Button>
      </span>
    {/snippet}
  </Tooltip>
{/snippet}

<!-- Every thread is the card a line comment is written in (DiffViewThread). Its state sits at the top right, in the badges of the
     tables (size lg), beside the name of the one who wrote it. -->
<div class={cx('flex max-w-(--comment-card) flex-col gap-3 rounded-lg border border-line-strong bg-surface-1 p-4 shadow-raised', resolved && 'opacity-70')}>
  <div>
    <div class="flex min-w-0 items-center gap-3">
      {@render byline(root)}

      {#if lines || root.outdated || resolved}
        <div class="flex shrink-0 items-center gap-2">
          {#if lines}<Badge tone="accent" size="lg">{lines}</Badge>{/if}
          {#if root.outdated}<span title={t('knowledge.drafts.thread.outdatedHint')}><Badge tone="warning" size="lg">{t('knowledge.drafts.thread.outdated')}</Badge></span>{/if}
          {#if resolved}<Badge tone="success" size="lg">{t('knowledge.drafts.thread.resolved')}</Badge>{/if}
        </div>
      {/if}
    </div>

    <!-- The lines a comment was written on, only where they are not on screen: in a list away from its section, or once they changed. Under its own line they would repeat the line above the card. -->
    {#if root.quote && root.quote.length > 0 && (named || root.outdated)}
      <div class="mt-2"><QuoteBlock lines={root.quote} /></div>
    {/if}

    <p class="mt-1 {COMMENT_TEXT}">{root.body}</p>
  </div>

  {#each replies as reply (idKey(reply.id))}
    <div class="border-l-2 border-line pl-3">{@render written(reply)}</div>
  {/each}

  {#if replying}
    <div class="flex flex-col gap-3">
      <GrowField bind:value={text} label={t('knowledge.drafts.thread.replyLabel')} placeholder={t('knowledge.drafts.thread.replyPlaceholder')} maxlength={4000} onsubmit={() => void send()} oncancel={() => (replying = false)} />

      <div class="flex flex-nowrap items-center gap-2">
        <Button variant="primary" disabled={text.trim() === '' || busy} onclick={() => void send()}>{t('knowledge.drafts.thread.reply')}</Button>
        <Button onclick={() => (replying = false)}>{t('common.cancel')}</Button>
      </div>
    </div>
  {:else}
    <div class="flex flex-nowrap items-center gap-2">
      {@render step(t('knowledge.drafts.thread.reply'), Reply, () => (replying = true))}
      {#if resolved}
        {@render step(t('knowledge.drafts.thread.reopen'), RotateCcw, () => onresolve(root.id, false))}
      {:else}
        {@render step(t('knowledge.drafts.thread.resolve'), Check, () => onresolve(root.id, true))}
      {/if}
      {#if mine(root)}
        {@render step(t('knowledge.drafts.thread.delete'), Trash2, () => ask(root), 'dangerQuiet')}
      {/if}
    </div>
  {/if}

  {#if readOnly}
    <p id={readOnlyId} class="flex items-center gap-2 text-sm text-fg-3"><Lock size={ICON.sm} class="shrink-0" aria-hidden="true" />{t('knowledge.panel.readOnly')}</p>
  {/if}

  {#if resolved && root.resolvedBy}
    <p class="text-xs text-fg-3">{t('knowledge.drafts.thread.resolvedBy', { name: who(root.resolvedBy) })}</p>
  {/if}
</div>

<ConfirmDialog
  bind:open={confirmOpen}
  title={t('knowledge.drafts.thread.deleteTitle')}
  description={deletingReplies > 0 ? tn('knowledge.drafts.thread.deleteReplies', deletingReplies) : t('knowledge.drafts.thread.deleteBody')}
  subject={deleting ? { icon: MessageSquare, name: deleting.body, details: who(deleting.by) } : undefined}
  confirmLabel={t('knowledge.drafts.thread.delete')}
  onconfirm={confirmDelete}
/>
