<script lang="ts" generics="T extends { harness: HarnessId }">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import PackageOpen from '@lucide/svelte/icons/package-open';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import type { Snippet } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import type { ViewLoader } from '$lib/stores/view-loader.svelte';

  import { onboarding } from '../onboarding/onboarding.svelte';

  interface Props {
    loader: ViewLoader<T>;
    /** What the view shows, for messages: "configuration", "extensions". */
    noun: string;
    children: Snippet<[T]>;
  }

  let { loader, noun, children }: Props = $props();

  const active = $derived(harness.active);
  const data = $derived(loader.data && loader.data.harness === active?.info.id ? loader.data : null);

  // Reload whenever the harness or the project changes; a no-op when the pair is already loaded.
  $effect(() => {
    loader.sync(active?.info.id ?? null, project.active?.path ?? null);
  });
</script>

{#if harness.status === 'idle' || harness.status === 'loading'}
  {@render loading()}
{:else if !active}
  <div class="grid size-full place-items-center">
    <EmptyState icon={PackageOpen} title="No harness found" body="Harness Desktop works with OMP and PI. Run the setup to detect them." />
  </div>
{:else if !active.detection.installed && !active.detection.configDirExists}
  <div class="grid size-full place-items-center">
    <EmptyState icon={PackageOpen} title="{active.info.name} is not installed" body="Install it, or point Harness Desktop at an existing binary, to see its {noun} here.">
      {#snippet action()}
        <Button variant="primary" onclick={() => onboarding.restart()}>Set up {active.info.shortName}</Button>
      {/snippet}
    </EmptyState>
  </div>
{:else if loader.status === 'error' && !data}
  <div class="grid size-full place-items-center">
    <EmptyState icon={CircleAlert} title="Could not read the {noun}" body={loader.error?.message}>
      {#snippet action()}
        <Button onclick={() => loader.refresh()}>Try again</Button>
      {/snippet}
    </EmptyState>
  </div>
{:else if data}
  <div class="flex size-full min-h-0 flex-col">
    {#if !active.detection.installed}
      <div class="mx-6 mt-4 flex items-center gap-3 rounded-md border border-line bg-warning-wash px-3.5 py-2.5 text-sm text-fg" role="status">
        <TriangleAlert class="shrink-0 text-warning" size={16} aria-hidden="true" />
        <span class="flex-1">{active.info.name} is not installed; showing what is on disk.</span>
        <Button size="sm" onclick={() => onboarding.restart()}>Set up {active.info.shortName}</Button>
      </div>
    {/if}

    {@render children(data)}
  </div>
{:else}
  {@render loading()}
{/if}

{#snippet loading()}
  <div class="flex flex-col gap-3 p-8" role="status" aria-label="Loading {noun}">
    <Skeleton width="14rem" height="1.75rem" />
    <Skeleton height="2.5rem" />
    <Skeleton height="2.5rem" />
    <Skeleton height="2.5rem" />
  </div>
{/snippet}

<span class="sr-only" aria-live="polite">{loader.status === 'loading' && data ? `Refreshing ${noun}` : ''}</span>
