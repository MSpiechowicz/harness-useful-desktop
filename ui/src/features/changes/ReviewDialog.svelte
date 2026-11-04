<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import FinishReviewDialog from '$lib/components/FinishReviewDialog.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { MAX_TEXT_BYTES, review } from '$lib/stores/review.svelte';

  interface Props {
    open: boolean;
    project: string;
    /** The review is ready to send: the dialog closes and the send step opens. */
    onpreview: () => void;
  }

  let { open = $bindable(), project, onpreview }: Props = $props();

  let confirmDiscard = $state(false);

  const comments = $derived(review.commentsFor(project));

  // Deleting the last comment leaves nothing to review.
  $effect(() => {
    if (open && comments.length === 0) {
      open = false;
    }
  });

  function discard(): void {
    review.discard(project);
    open = false;
  }
</script>

<FinishReviewDialog
  bind:open
  {comments}
  note={review.noteFor(project)}
  maxBytes={MAX_TEXT_BYTES}
  onnote={(note) => review.setNote(project, note)}
  onedit={(id, body) => review.edit(project, id, body)}
  onremove={(id) => review.remove(project, id)}
  onjump={(comment) => {
    git.select(comment.path);
    open = false;
  }}
  submitLabel={t('changes.review.preview')}
  onsubmit={onpreview}
>
  {#snippet leading()}
    <Button variant="dangerQuiet" onclick={() => (confirmDiscard = true)}>{t('changes.review.discardAll')}</Button>
  {/snippet}
</FinishReviewDialog>

<ConfirmDialog
  bind:open={confirmDiscard}
  title={t('changes.review.discardTitle')}
  description={`${tn('changes.review.discardDescription', comments.length)} ${t('changes.review.discardBody')}`}
  confirmLabel={t('changes.review.discard')}
  cancelLabel={t('changes.review.keep')}
  onconfirm={discard}
/>
