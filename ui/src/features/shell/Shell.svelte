<script lang="ts">
  import { untrack } from 'svelte';

  import CommandPalette from '$lib/components/CommandPalette.svelte';
  import { PaneDrawer } from '$lib/components/pane-drawer.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { saveWorkspaceWhenReady } from '$lib/stores/workspace.svelte';
  import { ecosystem } from '$lib/stores/ecosystem.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { sessionDetail, sessionList } from '$lib/stores/sessions.svelte';
  import { ui } from '$lib/stores/ui-state.svelte';
  import { palette } from '$lib/stores/palette.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { updates } from '$lib/stores/updates.svelte';
  import { configView, extensionsView } from '$lib/stores/views.svelte';
  import { matchesShortcut } from '$lib/utils/shortcuts';

  import { intro, INTRO_LEAD_MS } from '../intro/intro-state.svelte';
  import { onboarding } from '../onboarding/onboarding-state.svelte';
  import { familyUpdater } from '../ecosystem/family-updater';
  import { extensionUpdater } from '../extensions/actions/extension-updater';
  import RestartConfirm from '../updates/RestartConfirm.svelte';
  import { jumpToChat, newChat } from './actions';
  import { buildCommands } from './commands';
  import ConnectionBanner from './ConnectionBanner.svelte';
  import Rail from './Rail.svelte';
  import { globalShortcuts, shortcutGroups, type ShortcutAction } from './shortcuts';
  import ShortcutsOverlay from './ShortcutsOverlay.svelte';
  import TopBar from './TopBar.svelte';
  import ViewHost from './ViewHost.svelte';
  import { VIEWS } from './views';

  /** How long after agent activity the sessions list is re-read. */
  const SESSIONS_SOON_MS = 1000;

  let shortcutsOpen = $state(false);

  const commands = $derived(buildCommands({ openShortcuts: () => (shortcutsOpen = true) }));

  const shortcuts = globalShortcuts({
    togglePalette: () => palette.toggle(),
    openShortcuts: () => (shortcutsOpen = !shortcutsOpen),
    goBack: () => nav.back(),
    goForward: () => nav.forward(),
    newChat: () => void newChat(),
    jumpToChat,
    cycleChat: (step) => {
      nav.go('chat');
      agentSessions.cycle(step);
    },
    toggleRail: () => ui.toggleRail(),
    togglePane: () => void PaneDrawer.toggleCurrent(),
    goTo: (index) => nav.go(VIEWS[index].id),
  });
  const groups = $derived(shortcutGroups(shortcuts));

  // Where the user is (page, open chats, the one showing) is saved as it changes, so the next launch reopens it.
  $effect(() => {
    void nav.current;
    void agentSessions.activeKey;
    for (const session of agentSessions.sessions) {
      void session.chat.session?.sessionId;
      void session.chat.items.length;
      void session.ended;
      void session.aiLabel;
      void session.userNamed;
    }

    saveWorkspaceWhenReady(agentSessions);
  });

  // The open chat decides the project: switching chats brings that chat's project along, and with it the git state, the
  // Changes view with its review comments, the sessions list and the rail's count. Only the chat (not the project)
  // re-runs this, so choosing another project by hand is not undone until the chat changes again.
  $effect(() => {
    const path = agentSessions.active?.info.project ?? null;

    untrack(() => {
      if (path !== null && project.active?.path !== path) {
        void project.open(path);
      }
    });
  });

  // The rail and the chat card show git state everywhere, not only in Changes.
  $effect(() => {
    git.ensure(project.active?.path ?? null);
  });

  // An agent working in the open project edits files and writes its session as it goes: the Changes count and the
  // sessions list follow in the background instead of waiting for their views to be opened again.
  // Counted as a number, so streamed text (a new `chat` on every batch) does not count as activity.
  const activity = $derived.by(() => {
    const path = project.active?.path ?? null;
    let total = 0;
    for (const session of agentSessions.sessions) {
      if (session.info.project === path) {
        total += session.toolRuns + session.settledCount;
      }
    }

    return total;
  });
  let seenActivity = 0;

  $effect(() => {
    const current = activity;
    // Closing a tab or switching project lowers the count without anything having changed on disk.
    const grew = current > seenActivity;
    seenActivity = current;

    if (grew) {
      untrack(() => {
        git.refreshSoon();
        sessionsSoon();
      });
    }
  });

  $effect(() => () => {
    if (sessionsTimer !== null) {
      clearTimeout(sessionsTimer);
    }
  });

  // Sessions follow agent activity a beat later than git; a turn writes several entries in quick succession.
  let sessionsTimer: ReturnType<typeof setTimeout> | null = null;
  function sessionsSoon(): void {
    if (sessionsTimer !== null) {
      clearTimeout(sessionsTimer);
    }

    sessionsTimer = setTimeout(() => {
      sessionsTimer = null;
      void sessionList.refresh({ quiet: true });
      if (isLive(sessionDetail.ref?.id ?? null)) {
        void sessionDetail.follow();
      }
    }, SESSIONS_SOON_MS);
  }

  /** Whether a saved session is one an open chat is still running, so its transcript grows. */
  function isLive(id: string | null): boolean {
    return id !== null && agentSessions.sessions.some((session) => session.chat.session?.sessionId === id && !session.ended);
  }

  // The update policy runs once the intro and setup have handed over to the shell, never during them.
  $effect(() => {
    if (intro.phase === 'done' && prefs.value && !onboarding.visible) {
      void updates.startup();
    }
  });

  // Extensions keep themselves current while the setting is on; the schedule stops the moment it is switched off.
  $effect(() => {
    if (intro.phase === 'done' && prefs.value?.autoUpdateExtensions && !onboarding.visible) {
      return extensionUpdater.start();
    }
  });

  // Installed Harness family members keep themselves current under the same setting. Every ecosystem reading (the local
  // one and the one with the GitHub releases folded in) is a chance to find a newer release; the updater tries each once.
  $effect(() => {
    if (intro.phase === 'done' && prefs.value?.autoUpdateExtensions && !onboarding.visible && ecosystem.status) {
      void familyUpdater.check();
    }
  });

  // The palette searches sessions, configuration and extensions; make sure they are loaded when it opens.
  $effect(() => {
    if (!palette.open) {
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
    if ((palette.open && shortcut.id !== 'palette') || (shortcutsOpen && shortcut.id !== 'shortcuts')) {
      return;
    }

    event.preventDefault();
    shortcut.run();
  }
</script>

<!-- Back from a terminal or an editor: a commit or edit made there shows up in the Changes count. -->
<svelte:window onkeydown={onKeydown} onfocus={() => git.refreshSoon()} />

<div class="relative z-10 flex h-full" data-intro={intro.phase} style:--intro-lead="{INTRO_LEAD_MS}ms" inert={onboarding.visible}>
  <Rail onshortcuts={() => (shortcutsOpen = true)} />

  <div class="relative flex min-w-0 flex-1 flex-col">
    <TopBar />

    <main class="intro-enter intro-scale flex min-h-0 flex-1 flex-col" style:--intro-delay="100ms">
      <ConnectionBanner />
      <ViewHost />
    </main>
  </div>
</div>

<RestartConfirm />
<CommandPalette bind:open={palette.open} {commands} />
<ShortcutsOverlay open={shortcutsOpen} {groups} onclose={() => (shortcutsOpen = false)} />
