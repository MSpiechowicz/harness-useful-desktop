<script lang="ts">
  import ExternalLink from '@lucide/svelte/icons/external-link';
  import Info from '@lucide/svelte/icons/info';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import Disclosure from '$lib/components/Disclosure.svelte';
  import Logo from '$lib/components/Logo.svelte';
  import { ICON } from '$lib/components/icon';
  import { rise } from '$lib/design/motion';
  import { openExternal } from '$lib/ipc/actions';
  import type { EcosystemItem } from '$lib/ipc/bindings/EcosystemItem';
  import { app } from '$lib/stores/app.svelte';

  import { itemState } from './ecosystem-items';
  import InlineCode from './InlineCode.svelte';
  import InstallSteps from './InstallSteps.svelte';

  interface Props {
    item: EcosystemItem;
    /** Position in the list; cards enter one after another. */
    index?: number;
  }

  let { item, index = 0 }: Props = $props();

  const status = $derived(itemState(item));
  const titleId = $derived(`ecosystem-${item.id}`);

  // A missing companion shows how to get it straight away; an installed one tucks the steps away.
  let stepsOpen = $state(false);
  $effect.pre(() => {
    stepsOpen = status === 'missing';
  });
</script>

<article
  aria-labelledby={titleId}
  in:rise={{ y: 14, delay: 120 + index * 80, duration: 420 }}
  class="flex min-w-0 flex-col gap-3 rounded-lg border border-line bg-surface-1 p-5 shadow-raised"
>
  <header class="flex items-start gap-3">
    <!-- The product's own family mark, on a tile like its app icon. -->
    <span class="grid size-10 shrink-0 place-items-center rounded-md border border-line bg-surface-2 shadow-raised" aria-hidden="true"><Logo variant="mark" product={item.id} size={18} /></span>

    <div class="flex min-w-0 flex-1 flex-col">
      <h3 id={titleId} class="truncate text-lg text-fg">{item.name}</h3>
      <p class="text-sm text-fg-2">{item.tagline}</p>
    </div>
  </header>

  <div class="flex flex-wrap items-center gap-1.5">
    {#if status === 'installed'}
      <Badge tone="success" dot>Installed</Badge>
      {#if item.version}<Badge tone="neutral" class="tabular">v{item.version}</Badge>{/if}
    {:else if status === 'missing'}
      <Badge tone="neutral">Not installed</Badge>
    {:else}
      <Badge tone="warning">Not available on {app.info?.os ?? 'this system'}</Badge>
    {/if}
  </div>

  {#if item.via.length > 0}
    <ul class="flex flex-col gap-1" aria-label="Found as">
      {#each item.via as found (found)}
        <li class="rounded-sm bg-surface-2 px-2 py-1 font-mono text-xs [overflow-wrap:anywhere] text-fg-2">{found}</li>
      {/each}
    </ul>
  {/if}

  <p class="text-sm text-fg-2"><InlineCode text={item.description} /></p>

  {#if item.notes.length > 0}
    <ul class="flex flex-col gap-1.5">
      {#each item.notes as note (note)}
        <li class="flex items-start gap-2 text-sm text-fg-3">
          <Info size={16} class="mt-0.5 shrink-0" aria-hidden="true" />
          <span class="min-w-0 [overflow-wrap:anywhere]"><InlineCode text={note} /></span>
        </li>
      {/each}
    </ul>
  {/if}

  <div class="flex flex-wrap gap-1.5">
    <Button size="sm" onclick={() => void openExternal(item.docsUrl)}>
      {#snippet trailing()}<ExternalLink size={ICON.sm} aria-hidden="true" />{/snippet}
      Docs
      <span class="sr-only">for {item.name}, opens in your browser</span>
    </Button>
    <Button size="sm" variant="ghost" onclick={() => void openExternal(item.repoUrl)}>
      {#snippet trailing()}<ExternalLink size={ICON.sm} aria-hidden="true" />{/snippet}
      Repository
      <span class="sr-only">of {item.name}, opens in your browser</span>
    </Button>
  </div>

  {#if status === 'unsupported'}
    <p class="rounded-md border border-line bg-warning-wash px-3 py-2 text-sm text-fg" role="note">{item.name} does not run on this operating system, so there is nothing to install.</p>
  {:else if item.installSteps.length > 0}
    <Disclosure label={status === 'installed' ? 'Reinstall or update' : 'How to install'} meta="{item.installSteps.length} {item.installSteps.length === 1 ? 'step' : 'steps'}" bind:open={stepsOpen} class="border-t border-line pt-2">
      <InstallSteps steps={item.installSteps} />
    </Disclosure>
  {/if}
</article>
