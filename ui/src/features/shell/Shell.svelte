<script lang="ts">
  import CommandPalette from '$lib/components/CommandPalette.svelte';
  import { VIEWS } from './views';
  import { nav } from '$lib/stores/nav.svelte';
  import { matchesShortcut } from '$lib/utils/shortcuts';

  import { intro } from '../intro/intro.svelte';
  import { onboarding } from '../onboarding/onboarding.svelte';
  import { buildCommands } from './commands';
  import ConnectionBanner from './ConnectionBanner.svelte';
  import Rail from './Rail.svelte';
  import StatusBar from './StatusBar.svelte';
  import TopBar from './TopBar.svelte';
  import ViewHost from './ViewHost.svelte';

  let paletteOpen = $state(false);

  const commands = $derived(buildCommands());

  function isEditable(target: EventTarget | null): boolean {
    return target instanceof HTMLElement && (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName));
  }

  /** Global shortcuts. Each entry is tried in order; the first match wins. */
  const SHORTCUTS: { combo: string; allowInInputs: boolean; run: () => void }[] = [
    { combo: 'mod+k', allowInInputs: true, run: () => (paletteOpen = !paletteOpen) },
    { combo: 'mod+,', allowInInputs: true, run: () => nav.go('settings') },
    { combo: 'alt+arrowleft', allowInInputs: false, run: () => nav.back() },
    { combo: 'alt+arrowright', allowInInputs: false, run: () => nav.forward() },
    ...VIEWS.map((view, index) => ({ combo: `mod+${index + 1}`, allowInInputs: true, run: () => nav.go(view.id) })),
  ];

  function onKeydown(event: KeyboardEvent): void {
    // Setup is modal: the shell behind it takes no shortcuts.
    if (onboarding.visible) {
      return;
    }

    const shortcut = SHORTCUTS.find((entry) => matchesShortcut(event, entry.combo));
    if (!shortcut || (isEditable(event.target) && !shortcut.allowInInputs)) {
      return;
    }

    // While the palette is open only its own toggle applies.
    if (paletteOpen && shortcut.combo !== 'mod+k') {
      return;
    }

    event.preventDefault();
    shortcut.run();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="relative z-10 flex h-full" data-intro={intro.phase} inert={onboarding.visible}>
  <Rail />

  <div class="flex min-w-0 flex-1 flex-col">
    <TopBar onpalette={() => (paletteOpen = true)} />

    <main class="intro-enter intro-scale flex min-h-0 flex-1 flex-col" style:--intro-delay="100ms">
      <ConnectionBanner />
      <ViewHost />
    </main>

    <StatusBar />
  </div>
</div>

<CommandPalette bind:open={paletteOpen} {commands} />
