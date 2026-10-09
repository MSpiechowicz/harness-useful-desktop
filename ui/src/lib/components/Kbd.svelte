<script lang="ts">
  import { formatShortcut } from '$lib/utils/shortcuts';

  interface Props {
    /** Shortcut combo such as `mod+k`; rendered with platform-appropriate key caps. */
    keys: string;
    /** Every cap the same width (wide enough for "Shift"), for a column of shortcuts that should line up. */
    uniform?: boolean;
  }

  let { keys, uniform = false }: Props = $props();

  const caps = $derived(formatShortcut(keys));
</script>

<span class="inline-flex items-center gap-1" aria-label={caps.join(' ')}>
  {#each caps as cap (cap)}
    <kbd
      class={[
        'inline-flex items-center justify-center rounded-sm border border-line-strong bg-surface-2 font-sans leading-none font-medium text-fg-2 shadow-raised',
        uniform ? 'h-6 w-12 text-[11px]' : 'h-5 min-w-5 px-1.5 text-xs',
      ]}
      aria-hidden="true">{cap}</kbd
    >
  {/each}
</span>
