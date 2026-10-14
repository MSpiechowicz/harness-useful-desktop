<script lang="ts">
  import MessageSquareCode from '@lucide/svelte/icons/message-square-code';
  import MessageSquareText from '@lucide/svelte/icons/message-square-text';

  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
  import type { IconComponent } from './icon';

  interface Props {
    harness: HarnessId;
    size?: number;
    class?: string;
  }

  let { harness, size = 14, class: className }: Props = $props();

  // The Chat destination's bubble with a mark inside, one per harness, so the family is clear and the harness is told
  // apart by shape as well as colour.
  const ICONS: Record<HarnessId, IconComponent> = { omp: MessageSquareCode, pi: MessageSquareText };
  const Icon = $derived(ICONS[harness]);
</script>

<!-- Decorative: the harness name is always given in text next to it (or in the row's tooltip). -->
<span class={className} style:color="var(--harness-{harness})" aria-hidden="true">
  <Icon {size} />
</span>
