<script lang="ts">
  import type { Snippet } from 'svelte';

  import { t } from '$lib/i18n/i18n.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { HARNESS_SETUP_POLL_MS } from '$lib/stores/live';
  import { autoRefresh } from '$lib/utils/auto-refresh';

  interface Props {
    /** What stands in for the content while the machine is being read. */
    loading: Snippet;
    /** The content, once the scan has an answer. */
    children: Snippet;
  }

  let { loading, children }: Props = $props();

  // A harness installed in a terminal while a harness step is open turns up on its own.
  $effect(() => autoRefresh(() => void harness.rescan(), HARNESS_SETUP_POLL_MS));
</script>

{#if harness.status === 'loading' || harness.status === 'idle'}
  <div role="status" aria-label={t('onboarding.harnesses.looking')}>
    {@render loading()}
  </div>
{:else if harness.status === 'error'}
  <p class="rounded-md border border-line bg-danger-wash p-4 text-sm text-fg" role="alert">
    {t('onboarding.harnesses.error', { message: harness.error?.message })}
  </p>
{:else}
  {@render children()}
{/if}
