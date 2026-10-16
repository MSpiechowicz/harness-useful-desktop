<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import type { Snippet } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { RecommendedSetup } from '$lib/ipc/bindings/RecommendedSetup';

  import { setup } from './setup-state.svelte';

  interface Props {
    /** The height of the placeholder while the machine is being read. */
    placeholder: string;
    /** What the step shows once the machine has been read. */
    children: Snippet<[RecommendedSetup]>;
  }

  let { placeholder, children }: Props = $props();

  const data = $derived(setup.data);
</script>

<!-- The two steps read the same thing: a failed reading offers a retry, a pending one a placeholder, and only a reading shows the step. -->
{#if setup.load === 'error'}
  <div class="flex items-center gap-4 rounded-lg border border-line bg-danger-wash p-4" role="alert">
    <CircleAlert class="shrink-0 text-danger" size={18} aria-hidden="true" />
    <p class="min-w-0 flex-1 text-sm text-fg">{t('onboarding.setup.error', { message: setup.error })}</p>
    <Button onclick={() => void setup.open()}>{t('onboarding.setup.reload')}</Button>
  </div>
{:else if !data}
  <div role="status" aria-label={t('onboarding.setup.looking')}>
    <Skeleton height={placeholder} class="rounded-lg" />
  </div>
{:else}
  {@render children(data)}
{/if}
