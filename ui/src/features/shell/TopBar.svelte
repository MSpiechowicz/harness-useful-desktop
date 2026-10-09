<script lang="ts">
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';
  import Search from '@lucide/svelte/icons/search';

  import { SHELL_HEADER_SLOT } from '$lib/actions/portal';
  import IconButton from '$lib/components/IconButton.svelte';
  import { ICON } from '$lib/components/icon';
  import Kbd from '$lib/components/Kbd.svelte';
  import { nav } from '$lib/stores/nav.svelte';

  interface Props {
    onpalette: () => void;
  }

  let { onpalette }: Props = $props();
</script>

<!-- The one bar above the content: history, the current view's own header (title, state, actions), and search.
     Who and where you are (brand, harness, project) live on the rail. -->
<div class="intro-enter intro-top flex h-11 shrink-0 items-center gap-2 border-b border-line bg-surface-1/70 pr-3 pl-2" style:--intro-delay="60ms">
  <div class="flex shrink-0 items-center">
    <IconButton icon={ArrowLeft} label="Back" shortcut="alt+arrowleft" size="sm" side="bottom" disabled={!nav.canGoBack} onclick={() => nav.back()} />
    <IconButton icon={ArrowRight} label="Forward" shortcut="alt+arrowright" size="sm" side="bottom" disabled={!nav.canGoForward} onclick={() => nav.forward()} />
  </div>

  <!-- Stacked cell: during a page switch the outgoing and incoming headers crossfade in place. -->
  <div id={SHELL_HEADER_SLOT} class="grid h-full min-w-0 flex-1 grid-cols-[minmax(0,1fr)] *:col-start-1 *:row-start-1"></div>

  <button
    type="button"
    onclick={onpalette}
    aria-label="Search or run a command"
    class="inline-flex h-8 w-64 shrink-0 items-center gap-2 rounded-md border border-line bg-surface-2 px-3 text-sm whitespace-nowrap text-fg-3 transition-colors duration-[var(--dur-fast)] hover:border-line-strong hover:text-fg-2 active:scale-[0.99] max-[1280px]:w-44 max-[1040px]:w-auto"
  >
    <Search size={ICON.sm} aria-hidden="true" />
    <span class="flex-1 text-left max-[1040px]:hidden">Search or run a command</span>
    <span class="max-[1040px]:hidden"><Kbd keys="mod+k" /></span>
  </button>
</div>
