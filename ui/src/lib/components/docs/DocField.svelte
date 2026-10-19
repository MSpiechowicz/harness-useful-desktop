<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    label: string;
    /** The id of the control the label names. Without it the label is plain text and the control names itself (`labelId` carries the id). */
    for?: string;
    /** The id of the label itself, for a control that is not an input (a menu, a switch group). */
    labelId?: string;
    /** Something quiet at the far end of the label: "Optional", a character count. */
    aside?: string;
    /** What is wrong, said in red. It takes the place of `help`. */
    error?: string | null;
    /** A line on what the field is for. */
    help?: string;
    children: Snippet;
  }

  let { label, for: controlId, labelId, aside, error = null, help, children }: Props = $props();
</script>

<div class="flex flex-col gap-1.5">
  <div class="flex items-baseline justify-between gap-3 text-sm text-fg-2">
    {#if controlId}
      <label for={controlId}>{label}</label>
    {:else}
      <span id={labelId}>{label}</span>
    {/if}
    {#if aside}<small class="text-xs text-fg-3">{aside}</small>{/if}
  </div>

  {@render children()}

  {#if error}
    <p class="text-xs text-danger" role="alert">{error}</p>
  {:else if help}
    <p class="text-xs text-fg-3">{help}</p>
  {/if}
</div>
