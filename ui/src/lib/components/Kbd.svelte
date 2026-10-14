<script lang="ts">
  import { formatShortcut } from '$lib/utils/shortcuts';

  interface Props {
    /** Shortcut combo such as `mod+k`; rendered with platform-appropriate key caps. */
    keys: string;
    /** Every cap the same width (wide enough for "Shift"), for a column of shortcuts that should line up. */
    uniform?: boolean;
    /** Caps of one size for a list of shortcuts: 24px tall, a square for a single glyph (a letter, an arrow) and 48px wide for a word. */
    even?: boolean;
    /** Smaller caps (18px, 10px text), as on the dashboard's search field. */
    small?: boolean;
  }

  let { keys, uniform = false, even = false, small = false }: Props = $props();

  const caps = $derived(formatShortcut(keys));
</script>

<span class={['inline-flex items-center', small ? 'gap-[2px]' : 'gap-1']} aria-label={caps.join(' ')}>
  {#each caps as cap (cap)}
    <kbd
      class={[
        'inline-flex items-center justify-center border border-line-strong bg-surface-2 font-sans leading-none font-medium text-fg-2 shadow-raised',
        even ? ['h-6 rounded-sm text-[11px]', cap.length === 1 ? 'w-6' : 'w-12'] : uniform ? 'h-6 w-12 rounded-sm text-[11px]' : small ? 'h-[18px] min-w-[20px] rounded-[4px] px-[4px] text-[10px]' : 'h-5 min-w-5 rounded-sm px-1.5 text-xs',
      ]}
      aria-hidden="true"
      ><!-- Trimmed to the cap height, so the letters sit in the exact middle: Inter's line box has more room above
           capitals than below them. --><span class="block [text-box:trim-both_cap_alphabetic]">{cap}</span></kbd
    >
  {/each}
</span>
