<script lang="ts">
  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';

  import { ICON } from './icon';

  interface Props {
    harness: HarnessId;
    size?: number;
    class?: string;
  }

  let { harness, size = ICON.sm, class: className }: Props = $props();
</script>

<!-- Lucide's chat-with-a-dot bubble (the Chat destination's family): the outline follows the row's colour, the dot is the
     colour of the harness running the chat. Decorative: the harness name is always given in text next to it (or in the
     row's tooltip). -->
<svg class={className} width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <path d="M12.7 3H4a2 2 0 0 0-2 2v16.286a.71.71 0 0 0 1.212.502l2.202-2.202A2 2 0 0 1 6.828 19H20a2 2 0 0 0 2-2v-4.7" />
  <circle cx="19" cy="6" r="3.2" fill="var(--harness-{harness})" stroke="none" />
</svg>
