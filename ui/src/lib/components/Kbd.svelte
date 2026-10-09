<script lang="ts">
  import { formatShortcut } from '$lib/utils/shortcuts';

  interface Props {
    /** Shortcut combo such as `mod+k`; rendered with platform-appropriate key caps. */
    keys: string;
  }

  let { keys }: Props = $props();

  const caps = $derived(formatShortcut(keys));
</script>

<span class="inline-flex items-center gap-1" aria-label={caps.join(' ')}>
  {#each caps as cap (cap)}
    <kbd
      class="inline-flex h-5 min-w-5 items-center justify-center rounded-sm border border-line-strong bg-surface-2 px-1.5 font-sans text-xs leading-none font-medium text-fg-2 shadow-raised"
      aria-hidden="true">{cap}</kbd
    >
  {/each}
</span>
