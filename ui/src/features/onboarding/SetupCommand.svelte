<script lang="ts">
  import CopyButton from '$lib/components/CopyButton.svelte';

  interface Props {
    /** Shown exactly as given; the copy button puts the same text on the clipboard. */
    text: string;
    copyLabel: string;
  }

  let { text, copyLabel }: Props = $props();

  // Words and the white space between them: a word is one unit the line cannot break inside (a browser would break an
  // address after a hyphen), unless it is wider than the box.
  const lines = $derived(text.split('\n').map((line) => line.match(/\s+|\S+/g) ?? []));
</script>

<!-- Shown as text only: a command here is for the user's terminal or file, and the app runs only what the confirmation listed. It wraps at spaces, never inside a flag or an address (each word is one unit). -->
<div class="flex items-center gap-1 rounded-md border border-line bg-surface-2 py-0.5 pr-0.5 pl-3">
  <code class="block min-w-0 flex-1 py-1.5 font-mono text-xs leading-snug whitespace-pre-wrap text-fg">{#each lines as line, row (row)}<span class="block">{#each line as part, index (index)}{#if part.trim() === ''}{part}{:else}<span class="inline-block max-w-full [overflow-wrap:anywhere]">{part}</span>{/if}{/each}</span>{/each}</code>
  <CopyButton {text} label={copyLabel} control />
</div>
