<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';

  import Badge from '../Badge.svelte';
  import { ICON } from '../icon';

  interface Props {
    /** Why the latest refresh of the team documents failed; null when it did not. */
    problem: string | null;
    /** Team documents run without local approval. */
    trusted: boolean;
    /** The badge that says so. */
    trustedLabel: string;
  }

  let { problem, trusted, trustedLabel }: Props = $props();
</script>

<!-- One line above a documents table with what is true of the team's documents as a whole: they run unasked (a badge), and the last refresh failed (a note). -->
{#if problem || trusted}
  <div class="flex shrink-0 items-start gap-3 border-b border-line px-4 py-2.5 text-sm text-fg-2">
    {#if trusted}<Badge tone="info" class="shrink-0">{trustedLabel}</Badge>{/if}

    {#if problem}
      <p class="flex min-w-0 items-start gap-2" role="status"><CircleAlert size={ICON.sm} aria-hidden="true" class="mt-0.5 shrink-0 text-warning" /><span class="min-w-0 break-words">{problem}</span></p>
    {/if}
  </div>
{/if}
