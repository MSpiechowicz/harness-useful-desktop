<script lang="ts" module>
  /** The place in the Memory header that the list on screen fills with its own controls. */
  export const MEMORY_HEADER_ACTIONS = 'memory-header-actions';
</script>

<script lang="ts">
  import type { Snippet } from 'svelte';

  import { portal } from '$lib/actions/portal';

  interface Props {
    children: Snippet;
  }

  let { children }: Props = $props();
</script>

<!-- The header belongs to the whole view, so it stays while the list below it is swapped. The list on screen puts its
     search and its buttons into the header's actions through here. Without a header to receive them (a section rendered alone), they stay put. -->
<div class="contents" use:portal={MEMORY_HEADER_ACTIONS}>{@render children()}</div>
