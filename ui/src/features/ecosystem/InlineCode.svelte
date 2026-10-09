<script lang="ts">
  interface Props {
    /** Plain text in which `backticked` parts are set as code. Never parsed as HTML. */
    text: string;
  }

  let { text }: Props = $props();

  const parts = $derived(text.split('`').map((part, index) => ({ code: index % 2 === 1, part })));
</script>

{#each parts as { code, part }, index (index)}{#if code}<code class="rounded-sm bg-surface-3 px-1 py-0.5 font-mono text-[0.92em] text-fg-2">{part}</code>{:else}{part}{/if}{/each}
