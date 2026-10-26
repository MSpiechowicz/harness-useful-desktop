<script lang="ts">
  import { cx } from '$lib/utils/cx';
  import { overflowing } from '$lib/utils/overflowing';

  interface Props {
    /** Output text; may be empty while the command still runs. */
    text: string;
    /** The command line shown above the output. */
    command?: string | null;
    /** A very long command scrolls within a capped height (an approval keeps its buttons in view). */
    capCommand?: boolean;
    running?: boolean;
    class?: string;
  }

  let { text, command = null, capCommand = false, running = false, class: className }: Props = $props();

  let commandOverflows = $state(false);
</script>

<div class={cx('chat-radius overflow-hidden border border-line bg-canvas font-mono text-[13px] leading-relaxed', className)}>
  {#if command}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
    <div class={cx('border-b border-line px-3.5 py-2 break-all whitespace-pre-wrap text-fg', capCommand && 'max-h-48 overflow-y-auto')} tabindex={capCommand && commandOverflows ? 0 : undefined} {@attach capCommand && overflowing(command, (overflows) => (commandOverflows = overflows))}><span class="mr-2 text-accent-text select-none">$</span>{command}</div>
  {/if}

  {#if text !== '' || running}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
    <pre class="max-h-72 overflow-auto px-3.5 py-3 break-words whitespace-pre-wrap text-fg-2" tabindex="0">{text}{#if running}<span class="caret" aria-hidden="true"></span>{/if}</pre>
  {/if}
</div>
