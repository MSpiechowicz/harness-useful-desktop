<script lang="ts">
  interface Props {
    /** What the field says, shown exactly as given; it is text only and nothing here runs it. */
    text: string;
    /** A small heading above the field. */
    title?: string;
  }

  let { text, title }: Props = $props();
</script>

<div class="flex flex-col gap-1">
  {#if title}
    <p class="text-xs font-medium text-fg-3">{title}</p>
  {/if}
  <!-- A scrollable region must be reachable by keyboard; long lines wrap instead so nothing is cut off. -->
  <code class="block rounded-md border border-line bg-surface-2 px-3 py-2 font-mono text-xs leading-snug whitespace-pre-wrap [overflow-wrap:anywhere] text-fg">{text}</code>
</div>
