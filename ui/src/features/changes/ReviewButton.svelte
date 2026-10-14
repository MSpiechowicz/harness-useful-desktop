<script lang="ts">
  import MessageSquare from '@lucide/svelte/icons/message-square';

  import Badge from '$lib/components/Badge.svelte';
  import { ICON } from '$lib/components/icon';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { review } from '$lib/stores/review.svelte';

  import ReviewDialog from './ReviewDialog.svelte';
  import SendReviewDialog from './SendReviewDialog.svelte';

  interface Props {
    project: string;
  }

  let { project }: Props = $props();

  let open = $state(false);
  let sending = $state(false);

  const comments = $derived(review.commentsFor(project));
  const label = $derived(tn('changes.review.comments', comments.length));

  function preview(): void {
    open = false;
    sending = true;
  }
</script>

<!-- A yellow pill in the top bar, in the colour of the Changes count in the rail, once there is something to send. It opens
     the review: the files with comments on one side, their comments on the other, a search across both. -->
{#if comments.length > 0}
  <button
    type="button"
    aria-haspopup="dialog"
    aria-label={t('changes.review.heading', { label })}
    class="rounded-md focus-visible:outline-2 focus-visible:outline-offset-1"
    onclick={() => (open = true)}
  >
    <Badge tone="warning" class="h-control rounded-md! px-2.5 text-sm! transition-colors duration-[var(--dur-fast)] hover:bg-warning/20">
      <MessageSquare size={ICON.sm} aria-hidden="true" />
      {t('changes.review.section')}
      <span class="tabular rounded-full bg-warning/20 px-1.5 text-xs leading-5">{comments.length}</span>
    </Badge>
  </button>
{/if}

<ReviewDialog bind:open {project} onpreview={preview} />

{#if sending}
  <SendReviewDialog {project} onclose={() => (sending = false)} />
{/if}
