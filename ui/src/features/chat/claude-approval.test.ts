import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { i18n } from '$lib/i18n/i18n.svelte';
import type { ApprovalMode } from '$lib/ipc/bindings/ApprovalMode';
import type { NativeApprovalMode } from '$lib/ipc/bindings/NativeApprovalMode';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { harness } from '$lib/stores/harness.svelte';

import { chatSession } from '../../../tests/chat-helpers';
import ChatControls from './ChatControls.svelte';

/** The mock machine with Claude Code able to chat, and Yolo on or off for it. */
function withClaudeChat(yoloMode: boolean) {
  harness.summaries = fixtures.harnesses.map((summary) =>
    summary.info.id === 'claude' ? { ...summary, capabilities: { ...summary.capabilities, chat: true, yoloMode } } : summary,
  );
}

function claudeSession(nativeApprovalMode: NativeApprovalMode | null, approvalOverride: ApprovalMode | null = null) {
  return chatSession([], { harness: 'claude', configuredApprovalMode: null, approvalOverride, nativeApprovalMode }).session;
}

beforeEach(() => withClaudeChat(false));

afterEach(async () => {
  await i18n.load('en');
  harness.summaries = fixtures.harnesses;
});

describe('the approval modes offered in the chat header', () => {
  it('offers Claude Code its Auto mode and leaves Yolo out', async () => {
    render(ChatControls, { session: claudeSession(null) });

    await fireEvent.click(screen.getByRole('button', { name: /^Approval mode: Claude Code settings/ }));
    const choices = await screen.findAllByRole('menuitemradio');

    expect(choices.map((choice) => choice.textContent)).toEqual([
      expect.stringContaining('Use my configuration'),
      expect.stringContaining('ask every time'),
      expect.stringContaining('ask for writes'),
      expect.stringContaining('Claude Code decides which tool calls are safe and asks about the rest'),
    ]);
  });

  it('offers Auto to no other harness', async () => {
    render(ChatControls, { session: chatSession([], { harness: 'omp', configuredApprovalMode: 'yolo', approvalOverride: null }).session });

    await fireEvent.click(screen.getByRole('button', { name: /^Approval mode: yolo/ }));
    const choices = await screen.findAllByRole('menuitemradio');

    expect(choices).toHaveLength(4);
    expect(screen.queryByRole('menuitemradio', { name: /auto/ })).toBeNull();
  });

  it('shows Auto with the warning icon once a chat runs in it', () => {
    const { container } = render(ChatControls, { session: claudeSession(null, 'auto') });

    expect(screen.getByText('Approvals: auto')).toBeInTheDocument();
    expect(container.querySelector('.lucide-shield-alert')).not.toBeNull();
  });

  it('offers Yolo again once the harness can run without asking', async () => {
    withClaudeChat(true);
    render(ChatControls, { session: claudeSession(null) });

    await fireEvent.click(screen.getByRole('button', { name: /^Approval mode: Claude Code settings/ }));

    expect(await screen.findByRole('menuitemradio', { name: /yolo/ })).toBeInTheDocument();
  });
});

describe('the mode Claude Code is configured with', () => {
  it('shows its own mode, read-only, while the chat follows the settings', () => {
    render(ChatControls, { session: claudeSession({ name: 'acceptEdits', unattended: false }) });

    expect(screen.getByText('Claude Code: Accept edits')).toBeInTheDocument();
    expect(screen.queryByText(/Approvals:/)).toBeNull();
  });

  it('names a configured auto mode as the app\'s Auto', () => {
    const session = chatSession([], { harness: 'claude', configuredApprovalMode: 'auto', approvalOverride: null, nativeApprovalMode: { name: 'auto', unattended: true } }).session;
    const { container } = render(ChatControls, { session });

    expect(screen.getByText('Approvals: auto')).toBeInTheDocument();
    expect(container.querySelector('.lucide-shield-alert')).not.toBeNull();
  });

  it('marks a mode that runs tools unattended with the warning icon', () => {
    const { container } = render(ChatControls, { session: claudeSession({ name: 'bypassPermissions', unattended: true }) });

    expect(screen.getByText('Claude Code: Bypass permissions')).toBeInTheDocument();
    expect(container.querySelector('.lucide-shield-alert')).not.toBeNull();
  });

  it('gives way to the mode chosen for this chat', () => {
    render(ChatControls, { session: claudeSession({ name: 'bypassPermissions', unattended: true }, 'alwaysAsk') });

    expect(screen.getByText('Approvals: ask every time')).toBeInTheDocument();
    expect(screen.queryByText(/Bypass permissions/)).toBeNull();
  });

  it('shows nothing of its own when the settings choose no mode', () => {
    render(ChatControls, { session: claudeSession(null) });

    expect(screen.getByText('Approvals: Claude Code settings')).toBeInTheDocument();
    expect(screen.queryByText(/Claude Code: /)).toBeNull();
  });

  it('says a mode it does not know as the settings wrote it', () => {
    render(ChatControls, { session: claudeSession({ name: 'someNewMode', unattended: false }) });

    expect(screen.getByText('Claude Code: someNewMode')).toBeInTheDocument();
  });

  it('is worded in the language on screen', async () => {
    await i18n.load('pl');
    render(ChatControls, { session: claudeSession({ name: 'plan', unattended: false }) });

    expect(screen.getByText('Claude Code: Planowanie')).toBeInTheDocument();
  });
});
