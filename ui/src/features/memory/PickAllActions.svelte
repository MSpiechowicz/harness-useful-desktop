<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import Tooltip from '$lib/components/Tooltip.svelte';

  interface Props {
    selectAllLabel: string;
    selectNoneLabel: string;
    /** Why Tick all does nothing right now. Null when it can be pressed. */
    allReason: string | null;
    /** Why Untick all does nothing right now. Null when it can be pressed. */
    noneReason: string | null;
    onall: () => void;
    onnone: () => void;
  }

  let { selectAllLabel, selectNoneLabel, allReason, noneReason, onall, onnone }: Props = $props();
</script>

<!-- The two actions of a table of ticks: ordinary secondary buttons, and a disabled one says why on hover and focus. -->
<div class="flex items-center gap-2">
  <Tooltip text={allReason ?? ''} disabled={allReason === null}>
    {#snippet children({ describedby })}
      <Button aria-describedby={describedby} disabled={allReason !== null} onclick={onall}>{selectAllLabel}</Button>
    {/snippet}
  </Tooltip>

  <Tooltip text={noneReason ?? ''} disabled={noneReason === null}>
    {#snippet children({ describedby })}
      <Button aria-describedby={describedby} disabled={noneReason !== null} onclick={onnone}>{selectNoneLabel}</Button>
    {/snippet}
  </Tooltip>
</div>
