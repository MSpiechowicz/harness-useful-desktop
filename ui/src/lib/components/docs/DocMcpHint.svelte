<script lang="ts">
  import Info from '@lucide/svelte/icons/info';

  import { desktopMcp } from '$lib/stores/desktop-mcp.svelte';

  import Button from '../Button.svelte';
  import { ICON } from '../icon';

  interface Props {
    /** What agents write here ("Agents write hooks through…"), said while no harness has the MCP server. */
    message: string;
    /** The button that leads to the Recommended setup. */
    action: string;
    /** Runs before the Recommended setup opens, e.g. to close the dialog this sits in. */
    onleave?: () => void;
    /** Opens the Recommended setup. */
    onsetup: () => void;
  }

  let { message, action, onleave, onsetup }: Props = $props();

  // The machine is read once, when the hint first has a place on screen.
  $effect(() => {
    void desktopMcp.read();
  });
</script>

<!-- Agents write documents through the app's own MCP server. Without it in any harness there is nobody to write them, so the way to add it is one click away. -->
{#if desktopMcp.state === 'missing'}
  <div class="flex flex-wrap items-center justify-between gap-x-4 gap-y-2 rounded-md bg-info-wash px-3.5 py-2.5 text-sm" role="note">
    <p class="flex min-w-0 items-center gap-2 text-fg-2"><Info size={ICON.sm} class="shrink-0 text-info" aria-hidden="true" />{message}</p>
    <Button
      variant="ghost"
      onclick={() => {
        onleave?.();
        onsetup();
      }}>{action}</Button
    >
  </div>
{/if}
