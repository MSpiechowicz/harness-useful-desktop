<script lang="ts">
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';
  import Search from '@lucide/svelte/icons/search';

  import IconButton from '$lib/components/IconButton.svelte';
  import Kbd from '$lib/components/Kbd.svelte';
  import Logo from '$lib/components/Logo.svelte';
  import { nav } from '$lib/stores/nav.svelte';

  import HarnessSwitcher from './HarnessSwitcher.svelte';
  import ProjectSwitcher from './ProjectSwitcher.svelte';

  interface Props {
    onpalette: () => void;
  }

  let { onpalette }: Props = $props();
</script>

<header class="intro-enter intro-top flex h-12 shrink-0 items-center gap-3 border-b border-line bg-surface-1/70 px-4" style:--intro-delay="60ms">
  <Logo size={24} wordmark />

  <div class="mx-2 h-5 w-px bg-line-strong" aria-hidden="true"></div>

  <div class="flex items-center">
    <IconButton icon={ArrowLeft} label="Back" size="sm" side="bottom" disabled={!nav.canGoBack} onclick={() => nav.back()} />
    <IconButton icon={ArrowRight} label="Forward" size="sm" side="bottom" disabled={!nav.canGoForward} onclick={() => nav.forward()} />
  </div>

  <HarnessSwitcher />
  <ProjectSwitcher />

  <button
    type="button"
    onclick={onpalette}
    class="ml-auto inline-flex h-8 w-80 items-center gap-2 whitespace-nowrap rounded-md border border-line bg-surface-2 px-3 text-sm text-fg-3 transition-colors duration-[var(--dur-fast)] hover:border-line-strong hover:text-fg-2"
  >
    <Search size={14} aria-hidden="true" />
    <span class="flex-1 text-left">Search or run a command</span>
    <Kbd keys="mod+k" />
  </button>
</header>
