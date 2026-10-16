<script lang="ts" generics="T extends { harness: HarnessId }">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import PackageOpen from '@lucide/svelte/icons/package-open';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import type { Snippet } from 'svelte';

  import { fade } from 'svelte/transition';

  import Button from '$lib/components/Button.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { DURATION, scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import type { ViewLoader } from '$lib/stores/view-loader.svelte';

  import { onboarding } from '../onboarding/onboarding-state.svelte';

  interface Props {
    loader: ViewLoader<T>;
    /** Which view the gate guards; each has its own full sentences. */
    area: 'config' | 'extensions';
    children: Snippet<[T]>;
  }

  let { loader, area, children }: Props = $props();

  const active = $derived(harness.active);
  const data = $derived(loader.data && loader.data.harness === active?.info.id ? loader.data : null);
  // Another harness's view is still loaded: the user just switched. Its content is not shown, and the header keeps it until the new one is in.
  const switching = $derived(loader.data !== null && data === null);

  // Reload whenever the harness or the project changes; a no-op when the pair is already loaded.
  $effect(() => {
    loader.sync(active?.info.id ?? null, project.active?.path ?? null);
  });
</script>

{#if harness.status === 'idle' || harness.status === 'loading'}
  {@render loading()}
{:else if !active}
  <div class="grid size-full place-items-center">
    <EmptyState icon={PackageOpen} title={t('harness.gate.noneTitle')} body={t('harness.gate.noneBody')} />
  </div>
{:else if !active.detection.installed && !active.detection.configDirExists}
  <div class="grid size-full place-items-center">
    <EmptyState icon={PackageOpen} title={t('harness.gate.missingTitle', { name: active.info.name })} body={t(`harness.gate.${area}.missingBody`)}>
      {#snippet action()}
        <Button variant="primary" onclick={() => onboarding.restart()}>{t('harness.gate.setup', { name: active.info.shortName })}</Button>
      {/snippet}
    </EmptyState>
  </div>
{:else if loader.status === 'error' && !data}
  <div class="grid size-full place-items-center">
    <EmptyState icon={CircleAlert} title={t(`harness.gate.${area}.errorTitle`)} body={loader.error?.message}>
      {#snippet action()}
        <Button onclick={() => loader.refresh()}>{t('harness.gate.retry')}</Button>
      {/snippet}
    </EmptyState>
  </div>
{:else if data}
  <div class="flex size-full min-h-0 flex-col">
    {#if !active.detection.installed}
      <div class="mx-6 mt-4 flex items-center gap-3 rounded-md border border-line bg-warning-wash px-3.5 py-2.5 text-sm text-fg" role="status">
        <TriangleAlert class="shrink-0 text-warning" size={16} aria-hidden="true" />
        <span class="flex-1">{t('harness.gate.banner', { name: active.info.name })}</span>
        <Button onclick={() => onboarding.restart()}>{t('harness.gate.setup', { name: active.info.shortName })}</Button>
      </div>
    {/if}

    {@render children(data)}
  </div>
{:else if switching}
  <!-- A switch is usually quick: the spinner fades in only when it is not, so a fast one does not blink. -->
  <div class="grid size-full place-items-center">
    <div in:fade={{ duration: scaled(DURATION.base), delay: 150 }}>
      <Spinner size={20} label={t(`harness.gate.${area}.loading`)} class="text-fg-3" />
    </div>
  </div>
{:else}
  {@render loading()}
{/if}

{#snippet loading()}
  <div class="flex flex-col gap-3 p-8" role="status" aria-label={t(`harness.gate.${area}.loading`)}>
    <Skeleton width="14rem" height="1.75rem" />
    <Skeleton height="2.5rem" />
    <Skeleton height="2.5rem" />
    <Skeleton height="2.5rem" />
  </div>
{/snippet}

<span class="sr-only" aria-live="polite">{loader.status === 'loading' && data ? t(`harness.gate.${area}.refreshing`) : ''}</span>
