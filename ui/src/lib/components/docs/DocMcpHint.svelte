<script lang="ts">
  import Settings2 from '@lucide/svelte/icons/settings-2';

  import { desktopMcp } from '$lib/stores/desktop-mcp.svelte';
  import { cx } from '$lib/utils/cx';

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
    /** Centred under an empty view's own text and button; off in a dialog, whose text starts at the left. */
    centered?: boolean;
  }

  let { message, action, onleave, onsetup, centered = true }: Props = $props();

  // The machine is read once, when the hint first has a place on screen.
  $effect(() => {
    void desktopMcp.read();
  });
</script>

<!-- Agents write documents through the app's own MCP server. Without it in any harness there is nobody to write them, so the way to add it is one click away. -->
{#if desktopMcp.state === 'missing'}
  <!-- Grey text and a secondary button, as the app says every other hint: no coloured box, and the button on its own line, never wrapped beside the text. -->
  <div class={cx('flex flex-col gap-3', centered ? 'items-center text-center' : 'items-start')} role="note">
    <p class="text-sm text-fg-3">{message}</p>
    <Button
      onclick={() => {
        onleave?.();
        onsetup();
      }}
    >
      {#snippet leading()}<Settings2 size={ICON.sm} aria-hidden="true" />{/snippet}
      {action}
    </Button>
  </div>
{/if}
