<script lang="ts">
  interface Props {
    /** Plain text in which `**starred**` parts are set strong. Never parsed as HTML. */
    text: string;
  }

  let { text }: Props = $props();

  const parts = $derived(text.split('**').map((part, index) => ({ strong: index % 2 === 1, part })));
</script>

{#each parts as { strong, part }, index (index)}{#if strong}<strong class="font-medium text-fg">{part}</strong>{:else}{part}{/if}{/each}
