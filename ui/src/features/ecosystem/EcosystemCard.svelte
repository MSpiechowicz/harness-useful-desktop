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
  const updating = $derived(status === 'installed' && item.updateAvailable);
  const titleId = $derived(`ecosystem-${item.id}`);

  // A lone step needs no count.
  const stepCount = $derived(item.installSteps.length > 1 ? `${item.installSteps.length} steps` : undefined);

  // The secondary facts stay out of the way until asked for.
  let detailsOpen = $state(false);

  // A missing or outdated companion shows its steps straight away; an up-to-date one tucks them away.
  let stepsOpen = $state(false);
  $effect.pre(() => {
    stepsOpen = status === 'missing' || updating;
  });
</script>

{#snippet details()}
  <Disclosure label="Details" bind:open={detailsOpen}>
    <div class="flex flex-col gap-3 pb-2">
      {#if item.via.length > 0}
        <dl class="flex items-baseline gap-3">
          <dt class="w-16 shrink-0 text-xs text-fg-3">Found at</dt>
          <dd class="flex min-w-0 flex-1 flex-col gap-1">
            {#each item.via as found (found)}
              <span class="truncate rounded-sm bg-surface-2 px-2 py-1 font-mono text-xs text-fg-2" title={found}>{found}</span>
            {/each}
          </dd>
        </dl>
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
    </div>
  </Disclosure>
{/snippet}

{#snippet install()}
  {#if status !== 'unsupported' && item.installSteps.length > 0}
    <Disclosure label={updating ? 'How to update' : status === 'installed' ? 'Reinstall or update' : 'How to install'} meta={stepCount} bind:open={stepsOpen}>
      <InstallSteps steps={item.installSteps} />
    </Disclosure>
  {/if}
{/snippet}

<article
  aria-labelledby={titleId}
  in:rise={{ y: 14, delay: 120 + index * 80, duration: 420 }}
  class="flex min-w-0 flex-col gap-4 rounded-lg border border-line bg-surface-1 p-5 shadow-raised"
>
  <header class="flex items-start gap-3">
    <!-- The product's own family mark, on a tile like its app icon. -->
    <span class="grid size-10 shrink-0 place-items-center rounded-md border border-line bg-surface-2 shadow-raised" aria-hidden="true"><Logo variant="mark" product={item.id} size={18} /></span>

    <div class="flex min-w-0 flex-1 flex-col gap-0.5">
      <div class="flex flex-wrap items-center justify-between gap-x-2 gap-y-1">
        <h3 id={titleId} class="max-w-full truncate text-lg text-fg">{item.name}</h3>

        <!-- The state sits beside the name, and drops under it only when the name needs the room. -->
        <div class="flex shrink-0 items-center gap-1.5">
          {#if updating}
            <Badge tone="accent" dot>Update available</Badge>
          {:else if status === 'installed'}
            <Badge tone="success" dot>Installed</Badge>
          {:else if status === 'missing'}
            <Badge tone="neutral">Not installed</Badge>
          {:else}
            <Badge tone="warning">Unavailable</Badge>
          {/if}
        </div>
      </div>

      <p class="text-sm text-fg-2">{item.tagline}</p>
    </div>
  </header>

  <div class="flex flex-wrap items-center gap-1.5">
    <Button size="sm" onclick={() => void openExternal(item.repoUrl)}>
      {#snippet trailing()}<ExternalLink size={ICON.sm} aria-hidden="true" />{/snippet}
      GitHub
      <span class="sr-only">repository of {item.name}, opens in your browser</span>
    </Button>

    {#if status === 'installed' && item.version}
      <span class="tabular ml-auto text-xs text-fg-3">
        v{item.version}{#if updating && item.latestVersion}<span aria-hidden="true"> → </span><span class="sr-only">, version </span><span class="font-medium text-accent-text">v{item.latestVersion}</span><span class="sr-only"> is available</span>{/if}
      </span>
    {/if}
  </div>

  {#if status === 'unsupported'}
    <p class="rounded-md border border-line bg-warning-wash px-3 py-2 text-sm text-fg" role="note">{item.name} does not run on this operating system, so there is nothing to install.</p>
  {/if}

  <div class="flex flex-col gap-1 border-t border-line pt-2">
    {@render details()}
    {@render install()}
  </div>
</article>
