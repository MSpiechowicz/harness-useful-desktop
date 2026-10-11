<script lang="ts">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import ChevronUp from '@lucide/svelte/icons/chevron-up';
  import Trash2 from '@lucide/svelte/icons/trash-2';

  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import IconButton from '$lib/components/IconButton.svelte';
  import { commentHeading } from '$lib/git/comment-range';
  import { git } from '$lib/stores/git.svelte';
  import { byteLength, MAX_TEXT_BYTES, review } from '$lib/stores/review.svelte';

  import SendReviewDialog from './SendReviewDialog.svelte';

  interface Props {
    project: string;
  }

  let { project }: Props = $props();

  let open = $state(false);
  let sending = $state(false);
  let confirmDiscard = $state(false);

  const comments = $derived(review.commentsFor(project));
  const note = $derived(review.noteFor(project));
  const noteTooLong = $derived(byteLength(note) > MAX_TEXT_BYTES);
  const label = $derived(`${comments.length} ${comments.length === 1 ? 'comment' : 'comments'}`);

  function discard(): void {
    review.discard(project);
    confirmDiscard = false;
    open = false;
  }
</script>

{#if comments.length > 0}
  <section aria-label="Review" class="flex max-h-[45%] min-h-0 flex-col border-t border-line-strong bg-surface-1">
    <div class="flex items-center gap-3 px-4 py-2">
      <button type="button" aria-expanded={open} class="flex items-center gap-2 rounded-sm px-1.5 py-0.5 text-sm font-medium text-fg transition-colors hover:bg-hover active:bg-press" onclick={() => (open = !open)}>
        {#if open}<ChevronDown size={16} aria-hidden="true" />{:else}<ChevronUp size={16} aria-hidden="true" />{/if}
        Review · {label}
      </button>

      <div class="ml-auto flex items-center gap-2">
        <Button size="sm" variant="dangerQuiet" onclick={() => (confirmDiscard = true)}>Discard</Button>
        <Button size="sm" variant="primary" disabled={noteTooLong} onclick={() => (sending = true)}>Preview &amp; send</Button>
      </div>
    </div>

    {#if open}
      <div class="flex min-h-0 flex-col gap-3 overflow-y-auto px-4 pb-3">
        <ul class="flex flex-col gap-1.5" aria-label="Pending comments">
          {#each comments as comment (comment.id)}
            <li class="flex items-start gap-2 rounded-md border border-line bg-surface-2 px-3 py-2">
              <div class="flex min-w-0 flex-1 flex-col gap-0.5">
                <button type="button" class="truncate text-left font-mono text-xs text-accent-text hover:underline" onclick={() => git.select(comment.path)}>
                  {comment.path} · {commentHeading(comment)}
                </button>
                <p class="text-sm break-words whitespace-pre-wrap text-fg">{comment.body}</p>
              </div>
              <IconButton icon={Trash2} label="Delete comment" size="sm" onclick={() => review.remove(project, comment.id)} />
            </li>
          {/each}
        </ul>

        <label class="flex flex-col gap-1 text-xs font-medium text-fg-2">
          Overall instruction (optional)
          <textarea
            value={note}
            oninput={(event) => review.setNote(project, event.currentTarget.value)}
            rows="2"
            placeholder="e.g. Fix these, then run the tests."
            class="w-full resize-none rounded-sm border border-line bg-surface-2 px-3 py-2 text-base font-normal text-fg outline-none placeholder:text-fg-3 focus:border-focus"
          ></textarea>
          {#if noteTooLong}<span class="text-danger" role="alert">Too long: the instruction holds at most {MAX_TEXT_BYTES / 1024} KiB.</span>{/if}
        </label>
      </div>
    {/if}
  </section>
{/if}

{#if sending}
  <SendReviewDialog {project} onclose={() => (sending = false)} />
{/if}

<Dialog bind:open={confirmDiscard} title="Discard this review?" description="The {label} you wrote will be deleted. This cannot be undone." size="sm">
  <p class="text-base text-fg-2">Nothing has been sent to an agent.</p>
  {#snippet footer()}
    <Button variant="ghost" onclick={() => (confirmDiscard = false)}>Keep comments</Button>
    <Button variant="danger" onclick={discard}>Discard</Button>
  {/snippet}
</Dialog>
