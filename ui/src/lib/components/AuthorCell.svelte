<script lang="ts">
  import type { AuthorRef } from '$lib/ipc/bindings/AuthorRef';

  import { authorLine, viaText } from '$lib/utils/author-format';

  interface Props {
    /** Who wrote it, or none when no name was known: the cell is then empty. */
    author: AuthorRef | null | undefined;
  }

  let { author }: Props = $props();

  const via = $derived(author ? viaText(author) : null);
</script>

<!-- The name, and quieter after it the client an agent wrote through: "Ada via claude-code". -->
{#if author}
  <span class="block truncate text-fg-2" title={authorLine(author)}>
    {author.name}{#if via}{' '}<span class="text-fg-3">{via}</span>{/if}
  </span>
{/if}
