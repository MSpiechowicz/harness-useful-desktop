<script lang="ts">
  import { revealHidden } from './script-format';

  interface Props {
    text: string;
  }

  let { text }: Props = $props();

  const segments = $derived(revealHidden(text).segments);
</script>

<!-- Invisible characters show as their escape on a red chip, so code cannot hide what it does. No whitespace between the
     tags: this sits inside <pre>, where every space counts. A lone carriage return also starts a new line here, as it does
     in a terminal, so the code after it is not hidden behind what comes before. -->
{#each segments as part, index (index)}{#if part.hidden}<span data-hidden-char class="rounded-sm bg-danger/15 px-0.5 font-semibold text-danger">{part.text}</span>{#if part.lineBreak}{'\n'}{/if}{:else}{part.text}{/if}{/each}
