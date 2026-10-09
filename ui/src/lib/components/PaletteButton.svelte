<script lang="ts">
  import Search from '@lucide/svelte/icons/search';

  import { palette } from '$lib/stores/palette.svelte';

  import { ICON } from './icon';
  import Kbd from './Kbd.svelte';
</script>

<!-- The search field of the top bar: a fixed-size button that opens the command palette. Every view header places it
     just before its own actions, so it always sits in the same spot. -->
<button
  type="button"
  onclick={() => (palette.open = true)}
  aria-label="Search or run a command"
  aria-haspopup="dialog"
  aria-expanded={palette.open}
  class="inline-flex h-8 w-48 shrink-0 items-center gap-2 rounded-md border border-line bg-surface-2 px-2.5 text-sm whitespace-nowrap text-fg-3 transition-colors duration-[var(--dur-fast)] hover:border-line-strong hover:text-fg-2 active:scale-[0.99] max-[1100px]:w-auto"
>
  <Search size={ICON.sm} class="shrink-0" aria-hidden="true" />
  <span class="min-w-0 flex-1 truncate text-left max-[1100px]:hidden">Search</span>
  <Kbd keys="mod+k" />
</button>
