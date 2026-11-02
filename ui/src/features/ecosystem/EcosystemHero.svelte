<script lang="ts">
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { rise } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { EcosystemItem } from '$lib/ipc/bindings/EcosystemItem';

  import EcosystemKernel from './EcosystemKernel.svelte';
  import Emphasis from './Emphasis.svelte';
  import { PLACEHOLDER_STAGE, WIDE_STAGE_PX } from './kernel/kernel-layout';

  interface Props {
    /** `null` while the detection is still running (or failed). */
    items: EcosystemItem[] | null;
    failed?: boolean;
  }

  let { items, failed = false }: Props = $props();

  // The hero's padding (24px a side, see its classes) and border (1px) around the stage: a hero narrower than the wide stage plus
  // these stacks the modules instead. Before the first measurement (zero) the wide stage shows.
  const INSET_PX = 50;

  let width = $state(0);
  const narrow = $derived(width > 0 && width < WIDE_STAGE_PX + INSET_PX);
</script>

<section
  bind:clientWidth={width}
  aria-labelledby="ecosystem-hero-title"
  in:rise={{ y: 12, duration: 420 }}
  class="relative isolate overflow-hidden rounded-xl border border-line bg-surface-1 p-6 shadow-float @5xl:grid @5xl:grid-cols-[minmax(0,1fr)_auto] @5xl:items-center @5xl:gap-8 @5xl:p-8"
>
  <div class="flex min-w-0 flex-col gap-3">
    <p class="text-xs font-semibold tracking-widest text-accent-text uppercase">Harness Useful</p>
    <h2 id="ecosystem-hero-title" class="text-3xl text-fg">{t('ecosystem.hero.title')}</h2>
    <p class="max-w-prose text-base text-fg-2"><Emphasis text={t('ecosystem.hero.body')} /></p>
    <!-- Once the family is read, the diagram's own status line says how much of it is installed. -->
    {#if !items}
      <p class="tabular text-sm text-fg-3" role="status">{failed ? t('ecosystem.hero.failed') : t('ecosystem.hero.looking')}</p>
    {/if}
  </div>

  <div class="mt-6 @5xl:mt-0">
    {#if items}
      <EcosystemKernel {items} {narrow} />
    {:else if !failed}
      <Skeleton width="{PLACEHOLDER_STAGE.width}px" height="{PLACEHOLDER_STAGE.height}px" class="mx-auto max-w-full rounded-lg" />
    {/if}
  </div>
</section>
