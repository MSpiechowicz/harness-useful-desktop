<script lang="ts">
  import { updates } from '$lib/stores/updates.svelte';

  import { progressLabel } from './copy';

  /** One row (label, bar, size) for the update band; the default stacks the label above the bar. */
  let { inline = false }: { inline?: boolean } = $props();

  const state = $derived(updates.state);
  const fraction = $derived(updates.fraction);
  const title = $derived(state.kind === 'installing' ? 'Installing' : 'Downloading');
  const detail = $derived(state.kind === 'installing' ? '' : progressLabel(state));
</script>

{#snippet bar()}
  <div
    role="progressbar"
    aria-label="Update progress"
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={fraction === null ? undefined : Math.round(fraction * 100)}
    class="relative h-1.5 w-full overflow-hidden rounded-full bg-surface-3"
  >
    {#if fraction === null}
      <span class="indeterminate keep-motion absolute inset-y-0 w-2/5 rounded-full bg-[image:var(--brand-fill)]"></span>
    {:else}
      <span class="absolute inset-y-0 left-0 rounded-full bg-[image:var(--brand-fill)] transition-[width] duration-[var(--dur-base)] ease-out" style:width="{fraction * 100}%"></span>
    {/if}
  </div>
{/snippet}

{#if inline}
  <div class="flex items-center gap-3 text-sm whitespace-nowrap">
    <span class="text-fg-2">{title}…</span>
    <div class="w-40">{@render bar()}</div>
    {#if detail}<span class="tabular text-fg-3">{detail}</span>{/if}
  </div>
{:else}
  <div class="flex flex-col gap-2">
    <div class="flex items-baseline justify-between gap-3 text-sm">
      <span class="text-fg-2">{title}…</span>
      <span class="tabular text-fg-3">{detail}</span>
    </div>
    {@render bar()}
  </div>
{/if}

<style>
  .indeterminate {
    animation: slide 1.4s var(--ease-out-expo) infinite;
  }

  @keyframes slide {
    from {
      left: -40%;
    }

    to {
      left: 100%;
    }
  }
</style>
