<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import { updates } from '$lib/stores/updates.svelte';

  /** The update's progress bar: fills with the download, sweeps while the size or the install time is unknown. */
  const fraction = $derived(updates.fraction);
</script>

<div
  role="progressbar"
  aria-label={t('updates.progress.label')}
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={fraction === null ? undefined : Math.round(fraction * 100)}
  class="relative h-1.5 w-full overflow-hidden rounded-full bg-line-strong"
>
  {#if fraction === null}
    <span class="indeterminate keep-motion absolute inset-y-0 w-2/5 rounded-full bg-[image:var(--brand-fill)]"></span>
  {:else}
    <span class="absolute inset-y-0 left-0 rounded-full bg-[image:var(--brand-fill)] transition-[width] duration-[var(--dur-base)] ease-out" style:width="{fraction * 100}%"></span>
  {/if}
</div>

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
