import type { SlashOutcome } from '$lib/chat/slash';
import type { ChatSession } from '$lib/chat/session.svelte';
import { clearTranscript } from '$lib/chat/reducer';
import { t } from '$lib/i18n/i18n.svelte';
import type { GuiTarget } from '$lib/ipc/bindings/GuiTarget';
import { revealPath } from '$lib/ipc/actions';
import { nav, type ViewId } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { hasAgentPicker } from './agent-picker';

/** Desktop views that stand in for terminal-only commands; `null` opens a picker in the chat header. */
const GUI_VIEWS: Partial<Record<GuiTarget, ViewId>> = {
  settings: 'settings',
  sessionTree: 'sessions',
  forkPicker: 'sessions',
  resumePicker: 'sessions',
};

function openGui(session: ChatSession, target: GuiTarget): void {
  const view = GUI_VIEWS[target];
  if (view) {
    nav.go(view);

    return;
  }

  if (target === 'modelPicker') {
    session.ui.modelPicker = true;
  } else if (target === 'thinkingPicker') {
    session.ui.thinkingPicker = true;
  } else if (target === 'agentPicker') {
    openAgentPicker(session);
  } else if (target === 'approvalMode') {
    session.ui.approvalChooser = true;
  }
}

/** Opens the agent picker, or says why it cannot open: no agent picker in this chat, or a reply still running. */
function openAgentPicker(session: ChatSession): void {
  if (!hasAgentPicker(session)) {
    toasts.push({ title: t('chat.toast.unsupported'), tone: 'info' });

    return;
  }

  if (session.streaming) {
    toasts.push({ title: t('chat.agent.locked'), tone: 'info' });

    return;
  }

  session.ui.agentPicker = true;
}

async function copyLastReply(text: string | null): Promise<void> {
  if (!text) {
    toasts.push({ title: t('chat.toast.nothingToCopy'), description: t('chat.toast.nothingToCopyBody'), tone: 'info' });

    return;
  }

  try {
    await navigator.clipboard.writeText(text);
    toasts.push({ title: t('chat.toast.copied'), tone: 'success' });
  } catch {
    toasts.push({ title: t('chat.toast.copyFailed'), description: t('chat.toast.copyFailedBody'), tone: 'warning' });
  }
}

async function runAction(session: ChatSession, outcome: Extract<SlashOutcome, { type: 'action' }>): Promise<void> {
  if (outcome.kind === 'getSessionStats') {
    session.ui.stats = true;

    return;
  }

  const reply = await session.send(outcome.action);
  if (!reply) {
    return;
  }

  if (outcome.kind === 'newSession') {
    session.apply(clearTranscript);
  } else if (outcome.kind === 'exportHtml' && reply.type === 'exported') {
    const path = reply.path;
    toasts.push({ title: t('chat.toast.exported'), description: path, tone: 'success', duration: 9000, action: { label: t('chat.toast.reveal'), run: () => void revealPath(path) } });
  } else if (outcome.kind === 'getLastAssistantText' && reply.type === 'text') {
    await copyLastReply(reply.text);
  }
}

/** Carries out a resolved slash command (everything except plain `send`, which the composer handles). */
export async function runSlash(session: ChatSession, outcome: SlashOutcome): Promise<void> {
  switch (outcome.type) {
    case 'insert':
      session.draft = outcome.text;

      return;

    case 'send':
      await session.submit(outcome.text, 'send');

      return;

    case 'ask':
      session.ui.ask = outcome.kind;

      return;

    case 'unsupported':
      toasts.push({ title: t('chat.toast.unsupported'), description: outcome.reason, tone: 'info' });

      return;

    case 'gui':
      openGui(session, outcome.target);

      return;

    case 'action':
      await runAction(session, outcome);
  }
}
