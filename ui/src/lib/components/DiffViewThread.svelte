<script lang="ts">
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import Pencil from '@lucide/svelte/icons/pencil';
  import Trash2 from '@lucide/svelte/icons/trash-2';

  import { t } from '$lib/i18n/i18n.svelte';
  import { commentHeading, type ThreadComment } from '$lib/git/comment-range';

  import Badge from './Badge.svelte';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import DiffViewComposer from './DiffViewComposer.svelte';
  import IconButton from './IconButton.svelte';

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

<!-- A saved comment keeps the card it was written in: the same width and corners, the same badge naming the lines. Edit and
     delete are always there, at the height of the badge beside them. -->
<ul class="flex flex-col gap-2" aria-label={t('components.diff.comments')}>
  {#each comments as comment (comment.id)}
    <li class="max-w-(--comment-card)">
      {#if editing === comment.id}
        <DiffViewComposer
          heading={commentHeading(comment)}
          initial={comment.body}
          saveLabel={t('components.diff.save')}
          {maxBytes}
          onsave={(body) => {
            onedit(comment.id, body);
            editing = null;
          }}
          oncancel={() => (editing = null)}
        />
      {:else}
        <div class="group/comment flex flex-col gap-2 rounded-lg border border-line-strong bg-surface-1 p-4 shadow-raised">
          <div class="flex items-center gap-2">
            <Badge tone="accent" class="h-control rounded-md! px-2.5 text-sm!">{commentHeading(comment)}</Badge>
            <span class="ml-auto flex items-center gap-2">
              <IconButton icon={Pencil} label={t('components.diff.edit')} iconSize={16} class="bg-surface-3! hover:bg-surface-4!" onclick={() => (editing = comment.id)} />
              <IconButton icon={Trash2} label={t('components.diff.delete')} iconSize={16} class="bg-danger-wash! text-danger! hover:bg-danger/25!" onclick={() => ask(comment)} />
            </span>
          </div>
          <p class="px-1 text-base leading-relaxed break-words whitespace-pre-wrap text-fg">{comment.body}</p>
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
