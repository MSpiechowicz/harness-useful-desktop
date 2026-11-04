<script lang="ts">
  import Spinner from '$lib/components/Spinner.svelte';

  interface Props {
    /** What is being read, for screen readers: "Reading the document". */
    label: string;
  }

  let { label }: Props = $props();
</script>

<!-- One spinner in the middle of a detail panel while its item is read (see `useReveal`), in place of sections that would arrive one by one. -->
<div class="flex flex-1 items-center justify-center py-16">
  <Spinner size={32} {label} class="text-fg-3" />
</div>
