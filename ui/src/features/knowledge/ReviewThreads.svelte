<script lang="ts">
  import type { KnowledgeComment } from '$lib/ipc/bindings/KnowledgeComment';
  import { idKey } from '$lib/ipc/knowledge-id';

  import ReviewThread from './ReviewThread.svelte';
  import type { ThreadProps } from './thread-props';

  interface Props extends ThreadProps {
    /** The comments that start a thread here: the overall comments of the reviews, and the ones whose lines this diff no longer shows. */
    roots: KnowledgeComment[];
    /** Each thread names its section beside its lines: the list holds threads of other sections than the one on show. */
    named?: boolean;
  }

  let { roots, named = false, all, readOnly, busy, onreply, onresolve, ondelete }: Props = $props();
</script>

<ul class="flex flex-col gap-3">
  {#each roots as root (idKey(root.id))}
    <li>
      <ReviewThread {root} {named} {all} {readOnly} {busy} {onreply} {onresolve} {ondelete} />
    </li>
  {/each}
</ul>
