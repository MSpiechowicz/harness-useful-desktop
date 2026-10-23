<script lang="ts">
  interface Props {
    /** Accessible name of the bar. */
    label: string;
    /** How far it is, from 0 to 1. Null while the size or the time is unknown: the bar sweeps instead of filling. */
    fraction: number | null;
  }

  let { label, fraction }: Props = $props();
</script>

<div
  role="progressbar"
  aria-label={label}
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
