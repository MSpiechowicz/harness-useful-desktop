<script lang="ts">
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import Pencil from '@lucide/svelte/icons/pencil';
  import Trash2 from '@lucide/svelte/icons/trash-2';

  import { commentHeading, type ThreadComment } from '$lib/git/comment-range';

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
</script>

<ul class="flex flex-col gap-2" aria-label="Comments">
  {#each comments as comment (comment.id)}
    <li class="rounded-md border border-line bg-surface-2">
      {#if editing === comment.id}
        <DiffViewComposer
          heading={commentHeading(comment)}
          initial={comment.body}
          saveLabel="Save"
          {maxBytes}
          onsave={(body) => {
            onedit(comment.id, body);
            editing = null;
          }}
          oncancel={() => (editing = null)}
        />
      {:else}
        <div class="flex items-start gap-2 px-3 py-2">
          <MessageSquare size={14} class="mt-1 shrink-0 text-accent-text" aria-hidden="true" />
          <div class="flex min-w-0 flex-1 flex-col gap-0.5">
            <p class="text-xs text-fg-3">{commentHeading(comment)}</p>
            <p class="text-base break-words whitespace-pre-wrap text-fg">{comment.body}</p>
          </div>
          <IconButton icon={Pencil} label="Edit comment" size="sm" onclick={() => (editing = comment.id)} />
          <IconButton icon={Trash2} label="Delete comment" size="sm" onclick={() => onremove(comment.id)} />
        </div>
      {/if}
    </li>
  {/each}
</ul>
