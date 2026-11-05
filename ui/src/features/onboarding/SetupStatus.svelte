<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import type { Snippet } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import LoadingSpinner from '$lib/components/LoadingSpinner.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { RecommendedSetup } from '$lib/ipc/bindings/RecommendedSetup';

  import { setup } from './setup-state.svelte';

  interface Props {
    /** The height class of the space kept while the machine is being read. */
    placeholder: string;
    /** What the step shows once the machine has been read. */
    children: Snippet<[RecommendedSetup]>;
  }

  let { placeholder, children }: Props = $props();

  const data = $derived(setup.data);
</script>

<!-- The two steps read the same thing: a failed reading offers a retry, a pending one a spinner, and only a reading shows the step. -->
{#if setup.load === 'error'}
  <div class="flex items-center gap-4 rounded-lg border border-line bg-danger-wash p-4" role="alert">
    <CircleAlert class="shrink-0 text-danger" size={18} aria-hidden="true" />
    <p class="min-w-0 flex-1 text-sm text-fg">{t('onboarding.setup.error', { message: setup.error })}</p>
    <Button onclick={() => void setup.open()}>{t('onboarding.setup.reload')}</Button>
  </div>
{:else if !data}
  <LoadingSpinner label={t('onboarding.setup.looking')} class={placeholder} />
{:else}
  {@render children(data)}
{/if}
