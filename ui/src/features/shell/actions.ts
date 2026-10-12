import { t } from '$lib/i18n/i18n.svelte';
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
    const composer = document.querySelector<HTMLTextAreaElement>('textarea[role="combobox"]');

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
    toasts.push({ title: t('shell.toast.noHarness.title'), description: t('shell.toast.noHarness.description'), tone: 'warning' });

    return;
  }

  if (!project.active) {
    toasts.push({ title: t('shell.toast.needProject.title'), description: t('shell.toast.needProject.description'), tone: 'info' });

    return;
  }

  if (await agentSessions.create(active.info.id, project.active.path)) {
    focusComposer();
  }
}

/**
 * Works in another project from the chat. A harness that can move a session (OMP `/move`) takes the chat along:
 * the tab, its messages and its draft stay, only the project changes. Otherwise the agent cannot leave its folder,
 * so an empty chat (nothing said, nothing typed) is replaced in its own tab and any other one stays, with the new
 * chat opening next to it. Several chats may share a project either way.
 */
export async function switchProject(path: string): Promise<void> {
  const current = agentSessions.active;
  if (current?.info.project === path) {
    return;
  }
  if (project.active?.path !== path && !(await project.open(path))) {
    return;
  }

  if (current && !current.ended && harness.get(current.info.harness)?.capabilities.moveSession) {
    if (await agentSessions.moveProject(current.key, path)) {
      focusComposer();
    }

    return;
  }

  const replaceable = current && current.info.project !== path && !current.streaming && current.chat.items.length === 0 && current.draft.trim() === '';
  const harnessId = current?.info.harness ?? harness.active?.info.id;
  if (!harnessId) {
    await newChat();

    return;
  }

  nav.go('chat');
  const fresh = await agentSessions.create(harnessId, path);
  if (fresh && replaceable) {
    await agentSessions.takeOver(current.key, fresh.key);
  }
  if (fresh) {
    focusComposer();
  }
}

/** Picks a folder with the native dialog and switches the chat to it. */
export async function switchToPickedProject(): Promise<void> {
  const picked = await project.pick();
  if (picked) {
    await switchProject(picked.path);
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
    toasts.push({ title: t('shell.toast.noSession'), tone: 'info' });

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
