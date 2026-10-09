<script lang="ts">
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import Search from '@lucide/svelte/icons/search';

  import IconButton from '$lib/components/IconButton.svelte';
  import { ICON } from '$lib/components/icon';
  import Kbd from '$lib/components/Kbd.svelte';
  import { nav } from '$lib/stores/nav.svelte';

  import HarnessSwitcher from './HarnessSwitcher.svelte';
  import ProjectSwitcher from './ProjectSwitcher.svelte';

  interface Props {
    onpalette: () => void;
  }

  let { onpalette }: Props = $props();
</script>

<!-- One slim context strip: history, the harness and project you are in, and search. The brand is on the rail and
     each view names itself in its own header, so neither repeats here. -->
<header class="intro-enter intro-top flex h-11 shrink-0 items-center gap-3 border-b border-line bg-surface-1/70 px-3" style:--intro-delay="60ms">
  <div class="flex items-center">
    <IconButton icon={ArrowLeft} label="Back" shortcut="alt+arrowleft" size="sm" side="bottom" disabled={!nav.canGoBack} onclick={() => nav.back()} />
    <IconButton icon={ArrowRight} label="Forward" shortcut="alt+arrowright" size="sm" side="bottom" disabled={!nav.canGoForward} onclick={() => nav.forward()} />
  </div>

  <!-- Where you are: harness, then project. Both are also the switchers. -->
  <nav aria-label="You are here" class="flex min-w-0 items-center gap-1.5">
    <HarnessSwitcher />
    <ChevronRight size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />
    <ProjectSwitcher />
  </nav>

  <button
    type="button"
    onclick={onpalette}
    aria-label="Search or run a command"
    class="ml-auto inline-flex h-8 w-72 items-center gap-2 rounded-md border border-line bg-surface-2 px-3 text-sm whitespace-nowrap text-fg-3 transition-colors duration-[var(--dur-fast)] hover:border-line-strong hover:text-fg-2 active:scale-[0.99] max-[1280px]:w-44 max-[1040px]:w-auto"
  >
    <Search size={ICON.sm} aria-hidden="true" />
    <span class="flex-1 text-left max-[1040px]:hidden">Search or run a command</span>
    <span class="max-[1040px]:hidden"><Kbd keys="mod+k" /></span>
  </button>
</header>
