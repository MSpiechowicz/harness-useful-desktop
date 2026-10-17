<script lang="ts">
  import type { Snippet } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { ViewStatus } from '$lib/stores/view-loader.svelte';

  interface Props {
    /** Whether the body of the open item was read. */
    loaded: boolean;
    status: ViewStatus;
    error: string | null;
    /** What a failed read says when the server gave no reason. */
    failed: string;
    /** Accessible name of the placeholder shown while the body is read. */
    loading: string;
    onretry: () => void;
    children: Snippet;
  }

  let { loaded, status, error, failed, loading, onretry, children }: Props = $props();
</script>

<!-- The body of a note or a script comes with a second read: until it is in, a placeholder; if it fails, the reason and a retry. -->
{#if loaded}
  {@render children()}
{:else if status === 'error'}
  <div class="flex flex-col items-start gap-3">
    <p class="text-sm text-danger" role="alert">{error ?? failed}</p>
    <Button onclick={onretry}>{t('memory.tryAgain')}</Button>
  </div>
{:else}
  <div class="flex flex-col gap-2" role="status" aria-label={loading}>
    <Skeleton height="1.25rem" width="40%" />
    <Skeleton height="1.25rem" />
    <Skeleton height="1.25rem" width="75%" />
  </div>
{/if}
