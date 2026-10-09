<script lang="ts">
  import { cx } from '$lib/utils/cx';

  interface Props {
    /** Output text; may be empty while the command still runs. */
    text: string;
    /** The command line shown above the output. */
    command?: string | null;
    running?: boolean;
    class?: string;
  }

  let { text, command = null, running = false, class: className }: Props = $props();
</script>

<div class={cx('overflow-hidden rounded-md border border-line bg-canvas font-mono text-[12.5px] leading-relaxed', className)}>
  {#if command}
    <div class="border-b border-line px-3 py-1.5 break-all text-fg"><span class="mr-2 text-accent-text select-none">$</span>{command}</div>
  {/if}

  {#if text !== '' || running}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
    <pre class="max-h-72 overflow-auto px-3 py-2 break-words whitespace-pre-wrap text-fg-2" tabindex="0">{text}{#if running}<span class="caret" aria-hidden="true"></span>{/if}</pre>
  {/if}
</div>
