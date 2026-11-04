<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import { commentHeading } from '$lib/git/comment-range';
  import type { CommentSide } from '$lib/ipc/bindings/CommentSide';

  import DiffViewComposer from './DiffViewComposer.svelte';

  interface Props {
    /** The saved comment being changed. */
    comment: { side: CommentSide; startLine: number; endLine: number; body: string };
    maxBytes: number;
    onsave: (body: string) => void;
    /** The edit is over, whether it was saved or cancelled. */
    onleave: () => void;
  }

  let { comment, maxBytes, onsave, onleave }: Props = $props();
</script>

<!-- A saved comment opened for editing: the composer it was written in, filled with its text. -->
<DiffViewComposer
  heading={commentHeading(comment)}
  initial={comment.body}
  saveLabel={t('components.diff.save')}
  {maxBytes}
  onsave={(body) => {
    onsave(body);
    onleave();
  }}
  oncancel={onleave}
/>
