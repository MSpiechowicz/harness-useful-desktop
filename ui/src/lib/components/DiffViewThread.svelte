<script lang="ts">
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import Pencil from '@lucide/svelte/icons/pencil';
  import Trash2 from '@lucide/svelte/icons/trash-2';

  import { t } from '$lib/i18n/i18n.svelte';
  import { commentHeading, type ThreadComment } from '$lib/git/comment-range';

  import Badge from './Badge.svelte';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import DiffViewEdit from './DiffViewEdit.svelte';
  import Button from './Button.svelte';
  import { ICON } from './icon';

  interface Props {
    comments: ThreadComment[];
    maxBytes: number;
    onedit: (id: number, body: string) => void;
    onremove: (id: number) => void;
  }

  let { comments, maxBytes, onedit, onremove }: Props = $props();

  let editing = $state<number | null>(null);
  /** The comment waiting for an answer to "are you sure?". */
  let deleting = $state<ThreadComment | null>(null);
  let confirmOpen = $state(false);

  function ask(comment: ThreadComment): void {
    deleting = comment;
    confirmOpen = true;
  }

  function confirmDelete(): void {
    if (deleting) {
      onremove(deleting.id);
    }
  }
</script>

<!-- A comment of the review being written keeps the card it was written in, in the layout of a sent comment's card (the knowledge
     review's ReviewThread): who wrote it and that it is not sent yet on the left, the lines it is on in a table-size badge at the top
     right, the text, then Edit and Delete as buttons. -->
<ul class="flex flex-col gap-2" aria-label={t('components.diff.comments')}>
  {#each comments as comment (comment.id)}
    <li class="max-w-(--comment-card)">
      {#if editing === comment.id}
        <DiffViewEdit {comment} {maxBytes} onsave={(body) => onedit(comment.id, body)} onleave={() => (editing = null)} />
      {:else}
        <div class="flex flex-col gap-3 rounded-lg border border-line-strong bg-surface-1 p-4 shadow-raised">
          <div>
            <div class="flex min-w-0 items-center gap-3">
              <p class="flex min-h-5 min-w-0 flex-1 items-baseline gap-1.5 text-sm text-fg-3">
                <span class="truncate font-medium text-fg">{t('components.diff.you')}</span>
                <span aria-hidden="true">·</span>
                <span class="shrink-0 text-xs">{t('components.diff.notSent')}</span>
              </p>
              <Badge tone="accent" size="lg" class="shrink-0">{commentHeading(comment)}</Badge>
            </div>
            <p class="mt-1 text-sm leading-relaxed break-words whitespace-pre-wrap text-fg-2">{comment.body}</p>
          </div>

          <div class="flex flex-nowrap items-center gap-2">
            <Button aria-label={t('components.diff.edit')} onclick={() => (editing = comment.id)}>
              {#snippet leading()}<Pencil size={ICON.sm} aria-hidden="true" />{/snippet}
              {t('components.diff.editShort')}
            </Button>
            <Button variant="dangerQuiet" aria-label={t('components.diff.delete')} onclick={() => ask(comment)}>
              {#snippet leading()}<Trash2 size={ICON.sm} aria-hidden="true" />{/snippet}
              {t('components.diff.deleteConfirm')}
            </Button>
          </div>
        </div>
      {/if}
    </li>
  {/each}
</ul>

<ConfirmDialog
  bind:open={confirmOpen}
  title={t('components.diff.deleteTitle')}
  description={t('components.diff.deleteBody')}
  subject={deleting ? { icon: MessageSquare, name: deleting.body, details: commentHeading(deleting) } : undefined}
  confirmLabel={t('components.diff.deleteConfirm')}
  onconfirm={confirmDelete}
/>
