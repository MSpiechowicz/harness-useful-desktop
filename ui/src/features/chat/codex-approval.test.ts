import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import type { ApprovalMode } from '$lib/ipc/bindings/ApprovalMode';
import type { NativeApprovalMode } from '$lib/ipc/bindings/NativeApprovalMode';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { baseSessionInfo } from '$lib/ipc/mock/opencode-chat';
import { harness } from '$lib/stores/harness.svelte';

import { chatSession } from '../../../tests/chat-helpers';
import ChatControls from './ChatControls.svelte';

beforeEach(() => {
  harness.summaries = fixtures.harnesses;
});

afterEach(() => {
  harness.summaries = fixtures.harnesses;
});

function codexSession(configuredApprovalMode: ApprovalMode | null, nativeApprovalMode: NativeApprovalMode | null, approvalOverride: ApprovalMode | null = null) {
  return chatSession([], { harness: 'codex', configuredApprovalMode, nativeApprovalMode, approvalOverride }).session;
}

describe('the approval mode of a Codex chat', () => {
  it('shows the configured mode like any harness with approval modes', () => {
    render(ChatControls, { session: codexSession('write', { name: 'on-request / workspace-write', unattended: false }) });

    expect(screen.getByText('Approvals: ask for writes')).toBeInTheDocument();
  });

  it('shows the mock chat the way Codex reports its defaults', () => {
    const { configuredApprovalMode, nativeApprovalMode } = baseSessionInfo('codex');

    render(ChatControls, { session: codexSession(configuredApprovalMode, nativeApprovalMode) });

    expect(screen.getByText('Approvals: ask for writes')).toBeInTheDocument();
  });

  it('names the approval policy and sandbox read-only when they have no mode of the app', () => {
    render(ChatControls, { session: codexSession(null, { name: 'on-request / read-only', unattended: false }) });

    expect(screen.getByText('Codex: on-request / read-only')).toBeInTheDocument();
    expect(screen.queryByText(/Approvals:/)).toBeNull();
  });

  it('warns when the settings run commands unattended', () => {
    const { container } = render(ChatControls, { session: codexSession(null, { name: 'on-request / danger-full-access', unattended: true }) });

    expect(screen.getByText('Codex: on-request / danger-full-access')).toBeInTheDocument();
    expect(container.querySelector('.lucide-shield-alert')).not.toBeNull();
  });

  it('warns for Yolo as for OMP and OpenCode', () => {
    const { container } = render(ChatControls, { session: codexSession('yolo', { name: 'never / danger-full-access', unattended: true }) });

    expect(screen.getByText('Approvals: yolo')).toBeInTheDocument();
    expect(container.querySelector('.lucide-shield-alert')).not.toBeNull();
  });

  it('offers Yolo and leaves Auto to Claude Code', async () => {
    render(ChatControls, { session: codexSession('write', null) });

    await fireEvent.click(screen.getByRole('button', { name: /^Approval mode: ask for writes/ }));
    const choices = await screen.findAllByRole('menuitemradio');

    expect(choices.map((choice) => choice.textContent)).toEqual([
      expect.stringContaining('Use my configuration'),
      expect.stringContaining('ask every time'),
      expect.stringContaining('ask for writes'),
      expect.stringContaining('yolo'),
    ]);
  });
});
