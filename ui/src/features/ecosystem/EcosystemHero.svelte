<script lang="ts">
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { rise } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { EcosystemItem } from '$lib/ipc/bindings/EcosystemItem';

  import Emphasis from './Emphasis.svelte';
  import { installedSummary, updatesSummary } from './ecosystem-items';
  import EcosystemOrbit from './EcosystemOrbit.svelte';

  interface Props {
    /** `null` while the detection is still running (or failed). */
    items: EcosystemItem[] | null;
    failed?: boolean;
  }

  let { items, failed = false }: Props = $props();
</script>

<section
  aria-labelledby="ecosystem-hero-title"
  in:rise={{ y: 12, duration: 420 }}
  class="relative isolate overflow-hidden rounded-xl border border-line bg-surface-1 p-6 shadow-float @3xl:grid @3xl:grid-cols-[minmax(0,1fr)_auto] @3xl:items-center @3xl:gap-8 @3xl:p-8"
>
  <div class="flex min-w-0 flex-col gap-3">
    <p class="text-xs font-semibold tracking-widest text-accent-text uppercase">Harness Useful</p>
    <h2 id="ecosystem-hero-title" class="text-3xl text-fg">{t('ecosystem.hero.title')}</h2>
    <p class="max-w-prose text-base text-fg-2"><Emphasis text={t('ecosystem.hero.body')} /></p>
    <p class="tabular text-sm text-fg-3" role="status">{items ? [installedSummary(items), updatesSummary(items)].filter(Boolean).join(' · ') : failed ? t('ecosystem.hero.failed') : t('ecosystem.hero.looking')}</p>
  </div>

  <div class="mt-6 @3xl:mt-0">
    {#if items}
      <EcosystemOrbit {items} />
    {:else if !failed}
      <Skeleton height="178px" class="mx-auto w-[240px] rounded-lg" />
    {/if}
  </div>
</section>
