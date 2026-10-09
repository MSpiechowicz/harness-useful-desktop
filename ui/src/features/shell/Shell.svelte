<script lang="ts">
  import CommandPalette from '$lib/components/CommandPalette.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { sessionList } from '$lib/stores/sessions.svelte';
  import { ui } from '$lib/stores/ui-state.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { updates } from '$lib/stores/updates.svelte';
  import { configView, extensionsView } from '$lib/stores/views.svelte';
  import { matchesShortcut } from '$lib/utils/shortcuts';

  import { intro } from '../intro/intro.svelte';
  import { onboarding } from '../onboarding/onboarding.svelte';
  import RestartConfirm from '../updates/RestartConfirm.svelte';
  import UpdateNotice from '../updates/UpdateNotice.svelte';
  import { jumpToChat, newChat } from './actions';
  import { buildCommands } from './commands';
  import ConnectionBanner from './ConnectionBanner.svelte';
  import Rail from './Rail.svelte';
  import { globalShortcuts, shortcutGroups, type ShortcutAction } from './shortcuts';
  import ShortcutsOverlay from './ShortcutsOverlay.svelte';
  import StatusBar from './StatusBar.svelte';
  import TopBar from './TopBar.svelte';
  import ViewHost from './ViewHost.svelte';
  import { VIEWS } from './views';

  let paletteOpen = $state(false);
  let shortcutsOpen = $state(false);

  const commands = $derived(buildCommands({ openShortcuts: () => (shortcutsOpen = true) }));

  const shortcuts = globalShortcuts({
    togglePalette: () => (paletteOpen = !paletteOpen),
    openShortcuts: () => (shortcutsOpen = !shortcutsOpen),
    goBack: () => nav.back(),
    goForward: () => nav.forward(),
    newChat: () => void newChat(),
    jumpToChat,
    toggleRail: () => ui.toggleRail(),
    goTo: (index) => nav.go(VIEWS[index].id),
  });
  const groups = $derived(shortcutGroups(shortcuts));

  // The rail and the status bar show git state everywhere, not only in Changes.
  $effect(() => {
    git.ensure(project.active?.path ?? null);
  });

  // The update policy runs once the intro and setup have handed over to the shell, never during them.
  $effect(() => {
    if (intro.phase === 'done' && prefs.value && !onboarding.visible) {
      void updates.startup();
    }
  });

  // The palette searches sessions, configuration and extensions; make sure they are loaded when it opens.
  $effect(() => {
    if (!paletteOpen) {
      return;
    }

    const harnessId = harness.active?.info.id ?? null;
    const path = project.active?.path ?? null;

    sessionList.sync(harnessId, path);
    configView.sync(harnessId, path);
    extensionsView.sync(harnessId, path);
  });

  function isEditable(target: EventTarget | null): boolean {
    return target instanceof HTMLElement && (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName));
  }

  /** The combo of the shortcut this key press is, if any. */
  function matchedCombo(event: KeyboardEvent, shortcut: ShortcutAction): string | undefined {
    return [shortcut.combo, ...(shortcut.also ?? [])].find((combo) => matchesShortcut(event, combo));
  }

  function allowedHere(event: KeyboardEvent, shortcut: ShortcutAction, combo: string): boolean {
    if (!isEditable(event.target) || shortcut.inInputs === 'all') {
      return true;
    }

    return shortcut.inInputs === 'modified' && combo.startsWith('mod+');
  }

  function onKeydown(event: KeyboardEvent): void {
    // Setup is modal: the shell behind it takes no shortcuts.
    if (onboarding.visible) {
      return;
    }

    const shortcut = shortcuts.find((entry) => matchedCombo(event, entry) !== undefined);
    const combo = shortcut ? matchedCombo(event, shortcut) : undefined;
    if (!shortcut || combo === undefined || !allowedHere(event, shortcut, combo)) {
      return;
    }

    // While an overlay is open only its own toggle applies.
    if ((paletteOpen && shortcut.id !== 'palette') || (shortcutsOpen && shortcut.id !== 'shortcuts')) {
      return;
    }

    event.preventDefault();
    shortcut.run();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="relative z-10 flex h-full" data-intro={intro.phase} inert={onboarding.visible}>
  <Rail />

  <div class="relative flex min-w-0 flex-1 flex-col">
    <TopBar onpalette={() => (paletteOpen = true)} onshortcuts={() => (shortcutsOpen = true)} />

    <UpdateNotice />

    <main class="intro-enter intro-scale flex min-h-0 flex-1 flex-col" style:--intro-delay="100ms">
      <ConnectionBanner />
      <ViewHost />
    </main>

    <StatusBar onshortcuts={() => (shortcutsOpen = true)} />
  </div>
</div>

<RestartConfirm />
<CommandPalette bind:open={paletteOpen} {commands} />
<ShortcutsOverlay open={shortcutsOpen} {groups} onclose={() => (shortcutsOpen = false)} />
