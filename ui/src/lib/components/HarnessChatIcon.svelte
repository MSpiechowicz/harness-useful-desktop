<script lang="ts">
  import MessageSquare from '@lucide/svelte/icons/message-square';

  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';

  interface Props {
    harness: HarnessId;
    size?: number;
    class?: string;
  }

  let { harness, size = 14, class: className }: Props = $props();
</script>

<!-- The same bubble as the Chat destination, in the colour of the harness that runs the chat. Decorative: the harness
     name is always given in text next to it (or in the row's tooltip). -->
<span class={className} style:color="var(--harness-{harness})" aria-hidden="true">
  <MessageSquare {size} />
</span>
