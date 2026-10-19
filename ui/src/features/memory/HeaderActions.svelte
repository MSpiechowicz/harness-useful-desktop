<script lang="ts" module>
  import { getContext, setContext } from 'svelte';

  const SLOT = Symbol('memory-header-actions');

  /**
   * Names the place in a view's header that the list on screen fills with its own controls. Each view has its own: while one
   * view fades out and the next fades in both headers are on the page, and a shared one would hand the new list's controls to
   * the header that is leaving.
   */
  export function provideHeaderActions(id: string): string {
    return setContext(SLOT, id);
  }
</script>

<script lang="ts">
  import type { Snippet } from 'svelte';

  import { portal } from '$lib/actions/portal';

  interface Props {
    children: Snippet;
  }

  let { children }: Props = $props();

  const slot = getContext<string | undefined>(SLOT);
</script>

<!-- The header belongs to the whole view, so it stays while the list below it is swapped. The list on screen puts its
     search and its buttons into the header's actions through here. Without a header to receive them (a section rendered alone), they stay put. -->
{#if slot}
  <div class="contents" use:portal={slot}>{@render children()}</div>
{:else}
  <div class="contents">{@render children()}</div>
{/if}
