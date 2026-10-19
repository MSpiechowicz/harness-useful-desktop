<script lang="ts">
  import Switch from '../Switch.svelte';
  import Tooltip from '../Tooltip.svelte';

  interface Props {
    /** Names the switch for assistive technology: "Turn off Morning review". */
    label: string;
    checked: boolean;
    /** Why the switch is off, or null when it works: a document that waits for approval or cannot run is never turned on by a click, as that would approve what the user has not seen. */
    lockReason: string | null;
    /** A change is being written, so a second click waits. */
    busy: boolean;
    /** In a row of a table: a click or key on the switch must not open the row. */
    inRow?: boolean;
    onchange: (checked: boolean) => void;
  }

  let { label, checked, lockReason, busy, inRow = false, onchange }: Props = $props();
</script>

{#snippet control()}
  <Tooltip text={lockReason ?? ''} side={inRow ? 'top' : 'bottom'} disabled={lockReason === null}>
    {#snippet children({ describedby })}
      <span aria-describedby={describedby} class="inline-flex">
        <Switch {label} disabled={lockReason !== null || busy} bind:checked={() => checked, onchange} />
      </span>
    {/snippet}
  </Tooltip>
{/snippet}

{#if inRow}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <span class="inline-flex" onclick={(event) => event.stopPropagation()} onkeydown={(event) => event.stopPropagation()}>
    {@render control()}
  </span>
{:else}
  {@render control()}
{/if}
