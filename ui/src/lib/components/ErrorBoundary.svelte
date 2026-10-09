<script lang="ts">
  import type { Snippet } from 'svelte';

  import ErrorPanel from './ErrorPanel.svelte';

  interface Props {
    /** What is inside, as the start of a sentence: "The Ecosystem page", "Harness Desktop". */
    area: string;
    /** `silent` drops a decorative layer that failed; the others show "Something went wrong". */
    variant?: 'view' | 'screen' | 'silent';
    children: Snippet;
  }

  let { area, variant = 'view', children }: Props = $props();

  // Logged to the webview console only: the message may quote user content, which the app log never records.
  function report(error: unknown): void {
    console.error(`[harness] ${area} failed while rendering`, error);
  }
</script>

<!--
  A render or effect error inside stays inside: without this, one bad value (say, a duplicate key in a
  dashboard list) aborts the whole update and leaves the app frozen mid-transition. Errors in event
  handlers and promises are not caught here; the stores turn those into their own error states.
-->
<svelte:boundary onerror={report}>
  {@render children()}

  {#snippet failed(error, reset)}
    {#if variant !== 'silent'}
      <ErrorPanel {area} {error} retry={reset} variant={variant === 'screen' ? 'screen' : 'view'} />
    {/if}
  {/snippet}
</svelte:boundary>
