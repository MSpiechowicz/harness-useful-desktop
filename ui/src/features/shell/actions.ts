import type { MotionPref } from '$lib/ipc/bindings/MotionPref';
import type { ThemePref } from '$lib/ipc/bindings/ThemePref';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';
import { resumeInChat } from '$lib/stores/session-actions';
import { sessionList } from '$lib/stores/sessions.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

const FOCUS_ATTEMPTS = 30;

/**
 * Focuses the chat composer once it exists. The chat view loads lazily after a view switch, so
 * this retries for a moment instead of assuming the textarea is already there.
 */
export function focusComposer(): void {
  let attempts = 0;

  function tryFocus(): void {
    const composer = document.querySelector<HTMLTextAreaElement>('textarea[aria-label="Message"]');

    if (composer) {
      composer.focus();
      composer.setSelectionRange(composer.value.length, composer.value.length);
    } else if (attempts < FOCUS_ATTEMPTS) {
      attempts += 1;
      requestAnimationFrame(tryFocus);
    }
  }

  requestAnimationFrame(tryFocus);
}

/** Starts a chat for the harness and project in use; explains what is missing when it cannot. */
export async function newChat(): Promise<void> {
  const active = harness.active;
  nav.go('chat');

  if (!active?.detection.installed) {
    toasts.push({ title: 'No harness to chat with', description: 'Run the setup to detect OMP or PI.', tone: 'warning' });

    return;
  }

  if (!project.active) {
    toasts.push({ title: 'Open a project first', description: 'The agent works inside one folder.', tone: 'info' });

    return;
  }

  if (await agentSessions.create(active.info.id, project.active.path)) {
    focusComposer();
  }
}

/** Goes to the chat that is open, composer ready. */
export function jumpToChat(): void {
  nav.go('chat');
  focusComposer();
}

/** Puts `/command ` in the composer of the active chat and focuses it. */
export function insertSlashCommand(name: string): void {
  const session = agentSessions.active;
  if (!session) {
    return;
  }

  session.draft = `/${name} `;
  nav.go('chat');
  focusComposer();
}

/** Resumes the newest saved session of the project in a chat. */
export async function resumeLastSession(): Promise<void> {
  const newest = sessionList.data?.[0];
  const path = project.active?.path;

  if (!newest || !path) {
    toasts.push({ title: 'No saved session to resume', tone: 'info' });

    return;
  }

  if (await resumeInChat(agentSessions, newest.ref, path)) {
    focusComposer();
  }
}

const THEME_ORDER: ThemePref[] = ['system', 'dark', 'light'];
const MOTION_ORDER: MotionPref[] = ['full', 'reduced', 'off'];

function nextOf<T>(order: readonly T[], current: T): T {
  return order[(order.indexOf(current) + 1) % order.length];
}

export function cycleTheme(): void {
  void prefs.update({ theme: nextOf(THEME_ORDER, prefs.value?.theme ?? 'system') });
}

export function cycleMotion(): void {
  void prefs.update({ motion: nextOf(MOTION_ORDER, prefs.value?.motion ?? 'full') });
}
