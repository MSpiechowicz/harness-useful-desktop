<script lang="ts">
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import Keyboard from '@lucide/svelte/icons/keyboard';
  import Search from '@lucide/svelte/icons/search';

  import IconButton from '$lib/components/IconButton.svelte';
  import { ICON } from '$lib/components/icon';
  import Kbd from '$lib/components/Kbd.svelte';
  import Logo from '$lib/components/Logo.svelte';
  import { nav } from '$lib/stores/nav.svelte';

  import HarnessSwitcher from './HarnessSwitcher.svelte';
  import ProjectSwitcher from './ProjectSwitcher.svelte';
  import { viewMeta } from './views';

  interface Props {
    onpalette: () => void;
    onshortcuts: () => void;
  }

  let { onpalette, onshortcuts }: Props = $props();

  const view = $derived(viewMeta(nav.current));
</script>

<header class="intro-enter intro-top flex h-12 shrink-0 items-center gap-3 border-b border-line bg-surface-1/70 px-4" style:--intro-delay="60ms">
  <Logo size={15} wordmark class="max-[1180px]:hidden" />
  <Logo size={15} class="min-[1181px]:hidden" />

  <div class="mx-1 h-5 w-px bg-line-strong max-[1180px]:hidden" aria-hidden="true"></div>

  <div class="flex items-center">
    <IconButton icon={ArrowLeft} label="Back" shortcut="alt+arrowleft" size="sm" side="bottom" disabled={!nav.canGoBack} onclick={() => nav.back()} />
    <IconButton icon={ArrowRight} label="Forward" shortcut="alt+arrowright" size="sm" side="bottom" disabled={!nav.canGoForward} onclick={() => nav.forward()} />
  </div>

  <!-- Where you are: harness, project, then the view. The first two are also the switchers. -->
  <nav aria-label="You are here" class="flex min-w-0 items-center gap-1.5">
    <HarnessSwitcher />
    <ChevronRight size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />
    <ProjectSwitcher />
    <ChevronRight size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />
    <span aria-current="page" class="inline-flex min-w-0 items-center gap-2 text-base font-medium text-fg">
      <view.icon size={ICON.md} strokeWidth={1.8} class="shrink-0 text-accent-text" aria-hidden="true" />
      <span class="truncate max-[1000px]:hidden">{view.label}</span>
    </span>
  </nav>

  <div class="ml-auto flex items-center gap-2">
    <button
      type="button"
      onclick={onpalette}
      aria-label="Search or run a command"
      class="inline-flex h-8 w-72 items-center gap-2 rounded-md border border-line bg-surface-2 px-3 text-sm whitespace-nowrap text-fg-3 transition-colors duration-[var(--dur-fast)] hover:border-line-strong hover:text-fg-2 active:scale-[0.99] max-[1280px]:w-44 max-[1040px]:w-auto"
    >
      <Search size={ICON.sm} aria-hidden="true" />
      <span class="flex-1 text-left max-[1040px]:hidden">Search or run a command</span>
      <span class="max-[1040px]:hidden"><Kbd keys="mod+k" /></span>
    </button>

    <IconButton icon={Keyboard} label="Keyboard shortcuts" shortcut="shift+?" size="sm" side="bottom" onclick={onshortcuts} />
  </div>
</header>
