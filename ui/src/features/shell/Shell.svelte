<script lang="ts">
  import { untrack } from 'svelte';

  import CommandPalette from '$lib/components/CommandPalette.svelte';
  import { WorkspaceState } from '$lib/components/workspace.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { saveWorkspaceWhenReady } from '$lib/stores/workspace.svelte';
  import { ecosystem } from '$lib/stores/ecosystem.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { GIT_POLL_MS, HARNESS_FOCUS_GAP_MS } from '$lib/stores/live';
  import { project } from '$lib/stores/project.svelte';
  import { sessionDetail, sessionList } from '$lib/stores/sessions.svelte';
  import { ui } from '$lib/stores/ui-state.svelte';
  import { palette } from '$lib/stores/palette.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { memoryChat } from '$lib/stores/memoryChat.svelte';
  import { memoryHeld } from '$lib/stores/memoryHeld.svelte';
  import { scriptRequests } from '$lib/stores/scriptRequests.svelte';
  import { updates } from '$lib/stores/updates.svelte';
  import { configView, extensionsView } from '$lib/stores/views.svelte';
  import { autoRefresh } from '$lib/utils/auto-refresh';
  import { matchesShortcut } from '$lib/utils/shortcuts';

  import { intro, INTRO_LEAD_MS } from '../intro/intro-state.svelte';
  import { onboarding } from '../onboarding/onboarding-state.svelte';
  import ScriptRequestDialog from '../memory/ScriptRequestDialog.svelte';
  import { familyUpdater } from '../ecosystem/family-updater';
  import { hooks } from '../hooks/hooks-data.svelte';
  import { openRequestedRun } from '../routines/routine-request';
  import { routines } from '../routines/routines-data.svelte';
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
    toggleList: () => void WorkspaceState.toggleCurrentList(),
    toggleDetails: () => void WorkspaceState.toggleCurrentDetails(),
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
      // The window of a global routine works in a folder of the routines' own, which is no project to open.
      if (path !== null && !routines.inWorkspace(path) && project.active?.path !== path) {
        void project.open(path);
      }
    });
  });

  // The rail and the chat card show git state everywhere, not only in Changes.
  $effect(() => {
    git.ensure(project.active?.path ?? null);
  });

  // And they stay current there too: edits made by hand or by another program change the count within a few seconds,
  // while the window is visible. Back in the window (a commit or edit made in a terminal or an editor) it reads at once.
  // This is the one poll, so the Changes view does not run a second. Without Changes open it only reads the status:
  // the diffs are not needed for a count. While git cannot be read it follows the retry schedule instead of its pace.
  $effect(() =>
    project.active ? autoRefresh(() => git.refresh({ quiet: true, statusOnly: nav.current !== 'changes' }), GIT_POLL_MS, { failing: git.loadStatus === 'error' }) : undefined,
  );

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

  // A release made after launch is found while the app stays open, not only at the next start.
  $effect(() => {
    if (intro.phase === 'done' && !onboarding.visible) {
      return updates.watch();
    }
  });

  // Extensions keep themselves current: the marketplace catalogs always, and (while the setting is on) the plugins and
  // packages they reveal. The schedule runs from the start; the setting is read at every look.
  $effect(() => {
    if (intro.phase === 'done' && !onboarding.visible) {
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

  // An agent that wants to run a script this computer has not approved asks wherever the user is, so the shell listens from the start.
  $effect(() => scriptRequests.start());

  // Saves an agent holds for review ("Ask before saving") wait in their chat's card and are counted by its Memory button, so the shell listens from the start.
  $effect(() => untrack(() => memoryHeld.start()));
  // The waiting saves are listed per open chat: a chat that starts is read at once, one that stops is forgotten.
  $effect(() => {
    const keys = memoryChat.keys();

    untrack(() => memoryHeld.follow(keys));
  });

  // Hooks an agent wrote wait for the user's approval, and the rail counts them wherever the user is, so the shell listens from the start.
  $effect(() => hooks.start());
  $effect(() => {
    const where = project.active?.path ?? null;

    untrack(() => hooks.follow(where));
  });

  // Routines an agent wrote wait for the user's approval, and a run can wait for an answer. The rail counts both wherever the user is, so the shell listens from the start.
  $effect(() => routines.start());
  $effect(() => {
    const where = project.active?.path ?? null;

    untrack(() => routines.follow(where));
  });

  // A click on a "needs you" notification asks to show the run: its chat opens as a tab, wherever the user is.
  $effect(() => {
    const runId = routines.openRequest;

    if (runId !== null) {
      routines.openRequest = null;
      void openRequestedRun(runId);
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

  /** The overlays toggle themselves, and are dialogs too, so a modal dialog does not hold them back. */
  const OVERLAY_SHORTCUTS = new Set(['palette', 'shortcuts']);

  /** Whether a modal dialog is open and not on its way out. */
  function modalOpen(): boolean {
    return [...document.querySelectorAll('[role="dialog"][aria-modal="true"]')].some((dialog) => !dialog.closest('[inert]'));
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

    // A modal dialog (an approval, a confirmation) owns the keyboard: the shell behind it neither switches view, nor moves
    // through history or chats, nor toggles its panels or takes focus. Escape stays with the dialog. A dialog that is
    // closing (inert while it fades out) no longer counts, so a key pressed right after closing it is not lost.
    if (!OVERLAY_SHORTCUTS.has(shortcut.id) && modalOpen()) {
      return;
    }

    event.preventDefault();
    shortcut.run();
  }
</script>

<!-- Back from an installer or a terminal: a harness installed there is found without anyone asking. -->
<svelte:window onkeydown={onKeydown} onfocus={() => void harness.rescanIfStale(HARNESS_FOCUS_GAP_MS)} />

<div class="relative z-10 flex h-full overflow-clip" data-intro={intro.phase} style:--intro-lead="{INTRO_LEAD_MS}ms" inert={onboarding.visible}>
  <Rail onshortcuts={() => (shortcutsOpen = true)} />

  <div class="relative flex min-w-0 flex-1 flex-col overflow-clip">
    <TopBar />

    <main class="intro-enter intro-scale flex min-h-0 flex-1 flex-col" style:--intro-delay="100ms">
      <ConnectionBanner />
      <ViewHost />
    </main>
  </div>
</div>

<RestartConfirm />
<ScriptRequestDialog />
<CommandPalette bind:open={palette.open} {commands} />
<ShortcutsOverlay open={shortcutsOpen} {groups} onclose={() => (shortcutsOpen = false)} />
