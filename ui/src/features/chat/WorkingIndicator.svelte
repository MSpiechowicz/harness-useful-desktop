<script lang="ts">
  import Spinner from '$lib/components/Spinner.svelte';

  interface Props {
    /** `Thinking` while a reply's hidden reasoning streams, `Working` otherwise. */
    label: string;
  }

  let { label }: Props = $props();

  const startedAt = Date.now();
  let seconds = $state(0);

  $effect(() => {
    const timer = setInterval(() => (seconds = Math.floor((Date.now() - startedAt) / 1000)), 1000);

    return () => clearInterval(timer);
  });
</script>

<!-- Shown while the agent works with nothing on screen yet, so a long think never looks like a stall. -->
<p class="mt-3 flex items-center gap-2 text-sm text-fg-3" role="status">
  <Spinner size={14} class="text-accent-text" />
  <span>{label}…</span>
  {#if seconds >= 3}<span class="tabular text-xs">{seconds}s</span>{/if}
</p>
