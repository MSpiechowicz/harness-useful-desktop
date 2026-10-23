import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { asApproval } from '$lib/chat/approval';
import type { UiRequest } from '$lib/ipc/bindings/UiRequest';
import type { UiResponse } from '$lib/ipc/bindings/UiResponse';

import { startedSession } from '../../../tests/chat-helpers';
import ApprovalCard from './ApprovalCard.svelte';
import UiRequestDialog from './UiRequestDialog.svelte';

async function mountDialog(kind: UiRequest['kind']) {
  const backend = await startedSession();
  const request: UiRequest = { id: 'req-1', kind };
  render(UiRequestDialog, { session: backend.session, request });

  const answers = (): UiResponse[] => backend.of('agent_ui_respond').map((call) => call.response);

  return { ...backend, request, answers };
}

describe('agent dialogs', () => {
  it('picks a select option as its value', async () => {
    const { answers } = await mountDialog({ type: 'select', title: 'Choose a template', options: ['Basic', 'Full'], details: ['Small', null], timeoutMs: null });

    expect(await screen.findByRole('dialog', { name: 'Choose a template' })).toBeInTheDocument();
    expect(screen.getByText('Small')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: /Full/ }));

    await waitFor(() => expect(answers()).toEqual([{ id: 'req-1', outcome: { type: 'value', value: 'Full' } }]));
  });

  it('picks a select option with its number key', async () => {
    const { answers } = await mountDialog({ type: 'select', title: 'Pick', options: ['One', 'Two', 'Three'], details: [null, null, null], timeoutMs: null });

    await fireEvent.keyDown(await screen.findByRole('button', { name: /Two/ }), { key: '3' });

    await waitFor(() => expect(answers()[0].outcome).toEqual({ type: 'value', value: 'Three' }));
  });

  it('answers a confirmation with confirmed true or false', async () => {
    const yes = await mountDialog({ type: 'confirm', title: 'Install hooks', message: 'Add a pre-commit hook?', timeoutMs: null });
    expect(await screen.findByText('Add a pre-commit hook?')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Yes' }));
    await waitFor(() => expect(yes.answers()[0].outcome).toEqual({ type: 'confirmed', confirmed: true }));

    const no = await mountDialog({ type: 'confirm', title: 'Second', message: 'Sure?', timeoutMs: null });
    await fireEvent.click((await screen.findAllByRole('button', { name: 'No' })).at(-1) as HTMLElement);
    await waitFor(() => expect(no.answers()[0].outcome).toEqual({ type: 'confirmed', confirmed: false }));
  });

  it('submits typed input as the value', async () => {
    const { answers } = await mountDialog({ type: 'input', title: 'Hook name', placeholder: 'pre-commit', timeoutMs: null });

    await fireEvent.input(await screen.findByPlaceholderText('pre-commit'), { target: { value: 'pre-push' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Submit' }));

    await waitFor(() => expect(answers()[0].outcome).toEqual({ type: 'value', value: 'pre-push' }));
  });

  it('starts an editor from its prefill and submits the edited text', async () => {
    const { answers } = await mountDialog({ type: 'editor', title: 'Edit the hook', prefill: '#!/bin/sh\n' });

    const editor = (await screen.findByRole('textbox')) as HTMLTextAreaElement;
    expect(editor.value).toBe('#!/bin/sh\n');

    await fireEvent.input(editor, { target: { value: '#!/bin/sh\ncargo fmt\n' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Submit' }));

    await waitFor(() => expect(answers()[0].outcome).toEqual({ type: 'value', value: '#!/bin/sh\ncargo fmt\n' }));
  });

  it('treats Escape as cancelled', async () => {
    const { answers } = await mountDialog({ type: 'input', title: 'Hook name', placeholder: null, timeoutMs: null });

    await fireEvent.keyDown(await screen.findByRole('dialog'), { key: 'Escape' });

    await waitFor(() => expect(answers()).toEqual([{ id: 'req-1', outcome: { type: 'cancelled' } }]));
  });

  it('counts down when the request has a timeout', async () => {
    await mountDialog({ type: 'confirm', title: 'Quick', message: 'Hurry', timeoutMs: 30000 });

    expect(await screen.findByRole('timer')).toHaveTextContent('30s');
  });
});

const APPROVAL: UiRequest = {
  id: 'approval-1',
  kind: { type: 'select', title: 'Allow bash: cargo test?', options: ['Approve', 'Deny'], details: ['Runs once', null], timeoutMs: null },
};

async function mountApproval() {
  const backend = await startedSession();
  backend.session.ingest({ sessionKey: backend.session.key, seq: 1, event: { type: 'toolStart', toolCallId: 'c1', name: 'bash', args: { command: 'cargo test' }, intent: 'Run the tests' } });
  backend.session.ingest({ sessionKey: backend.session.key, seq: 2, event: { type: 'uiRequest', request: APPROVAL } });
  backend.session.flush();

  const prompt = asApproval(APPROVAL)!;
  render(ApprovalCard, { session: backend.session, prompt });

  return { ...backend, answers: () => backend.of('agent_ui_respond').map((call) => call.response.outcome) };
}

describe('approval card', () => {
  it('names the tool and shows the exact command being approved', async () => {
    await mountApproval();

    expect(screen.getByRole('region', { name: 'Approval needed' })).toBeInTheDocument();
    expect(screen.getByText('Allow bash: cargo test?')).toBeInTheDocument();
    expect(screen.getByText('cargo test')).toBeInTheDocument();
    expect(screen.getByText('Run the tests')).toBeInTheDocument();
  });

  it('approves and denies with its buttons', async () => {
    const approve = await mountApproval();
    await fireEvent.click(screen.getByRole('button', { name: /^Approve/ }));
    await waitFor(() => expect(approve.answers()).toEqual([{ type: 'value', value: 'Approve' }]));
    expect(approve.session.approval).toBeNull();

    const deny = await mountApproval();
    await fireEvent.click(screen.getAllByRole('button', { name: /^Deny/ }).at(-1) as HTMLElement);
    await waitFor(() => expect(deny.answers()).toEqual([{ type: 'value', value: 'Deny' }]));
  });

  it('approves with Alt+Y, even where Option+Y types a symbol', async () => {
    const { answers } = await mountApproval();

    await fireEvent.keyDown(window, { key: '¥', code: 'KeyY', altKey: true });

    await waitFor(() => expect(answers()).toEqual([{ type: 'value', value: 'Approve' }]));
  });

  it('denies with Alt+N and with Escape', async () => {
    const alt = await mountApproval();
    await fireEvent.keyDown(window, { key: 'n', code: 'KeyN', altKey: true });
    await waitFor(() => expect(alt.answers()).toEqual([{ type: 'value', value: 'Deny' }]));
  });

  it('denies with Escape', async () => {
    const { answers } = await mountApproval();

    await fireEvent.keyDown(window, { key: 'Escape' });

    await waitFor(() => expect(answers()).toEqual([{ type: 'value', value: 'Deny' }]));
  });
});

/** Starts tool calls as given, opens `request` and shows its approval card. */
async function mountBound(calls: { id: string; name: string; args: Record<string, string> }[], request: UiRequest) {
  const backend = await startedSession();
  let seq = 0;
  for (const call of calls) {
    backend.session.ingest({ sessionKey: backend.session.key, seq: ++seq, event: { type: 'toolStart', toolCallId: call.id, name: call.name, args: call.args, intent: null } });
  }
  backend.session.ingest({ sessionKey: backend.session.key, seq: ++seq, event: { type: 'uiRequest', request } });
  backend.session.flush();

  render(ApprovalCard, { session: backend.session, prompt: asApproval(request)! });

  return backend;
}

const permission = (id: string, title: string, toolCallId?: string): UiRequest => ({
  id,
  kind: { type: 'select', title, options: ['Allow once', 'Always allow', 'Reject'], details: [null, null, null], timeoutMs: null },
  purpose: 'approval',
  toolCallId,
});

describe('approval card bound to its tool call', () => {
  const parallel = [
    { id: 'call_a', name: 'bash', args: { command: 'cargo test' } },
    { id: 'call_b', name: 'bash', args: { command: 'git push --force origin main' } },
  ];

  it('shows the command of the call that asks, not the newest one', async () => {
    await mountBound(parallel, permission('per_1', 'Allow bash: cargo test?', 'call_a'));

    expect(screen.getByText('cargo test')).toBeInTheDocument();
    expect(screen.queryByText('git push --force origin main')).toBeNull();
  });

  it('shows each parallel ask with its own command', async () => {
    await mountBound(parallel, permission('per_2', 'Allow bash: git push --force origin main?', 'call_b'));

    expect(screen.getByText('git push --force origin main')).toBeInTheDocument();
    expect(screen.queryByText('cargo test')).toBeNull();
  });

  it('shows no other call when the one that asks is not in the transcript', async () => {
    await mountBound(parallel, permission('per_3', 'Allow bash for a subagent: ls -la?', 'call_sub'));

    expect(screen.getByText('Allow bash for a subagent: ls -la?')).toBeInTheDocument();
    expect(screen.queryByText('cargo test')).toBeNull();
    expect(screen.queryByText('git push --force origin main')).toBeNull();
  });

  it('shows a 300-character command in full', async () => {
    const command = `echo ${'x'.repeat(290)} && git push --force`;
    await mountBound([{ id: 'call_a', name: 'bash', args: { command } }], permission('per_1', 'Allow bash?', 'call_a'));

    expect(screen.getByText(command)).toBeInTheDocument();
  });

  it('never clamps a title that names more than the call shows, and keeps a command as written', async () => {
    const title = `Allow apply_patch: writing anywhere below /, ${Array.from({ length: 40 }, (_, index) => `src/file-${index}.rs`).join(', ')}?`;
    await mountBound([{ id: 'call_a', name: 'apply_patch', args: { path: '/work/src/file-0.rs' } }], permission('per_1', title, 'call_a'));

    const heading = screen.getByRole('heading', { name: title });
    expect(heading.className).not.toContain('line-clamp');
    expect(heading.className).toContain('overflow-y-auto');
  });

  it('shows a command over two lines with its spacing', async () => {
    const command = "printf  '%s'\nsecond line";
    await mountBound([{ id: 'call_a', name: 'bash', args: { command } }], permission('per_1', `Allow shell: ${command}?`, 'call_a'));

    const shown = screen.getByText((_, element) => element?.tagName === 'DIV' && element.textContent === `$${command}`);
    expect(shown.className).toContain('whitespace-pre-wrap');
  });

  it('shows a 300-character path in full', async () => {
    const path = `/work/${'deep/'.repeat(58)}secret.txt`;
    await mountBound([{ id: 'call_a', name: 'write', args: { path } }], permission('per_1', 'Allow write?', 'call_a'));

    expect(screen.getByText(path)).toBeInTheDocument();
  });
});

describe('questions', () => {
  const question: UiRequest = {
    id: 'que_1',
    kind: { type: 'select', title: 'Commit the change?', options: ['Yes', 'No', 'Type your own answer…'], details: [null, null, null], timeoutMs: null },
    purpose: 'question',
  };

  it('never reads as an approval, whatever its options say', () => {
    expect(asApproval(question)).toBeNull();
    expect(asApproval({ ...question, purpose: undefined })).not.toBeNull();
  });

  it('opens as a dialog with every option', async () => {
    const backend = await startedSession();
    backend.session.ingest({ sessionKey: backend.session.key, seq: 1, event: { type: 'uiRequest', request: question } });
    backend.session.flush();

    expect(backend.session.approval).toBeNull();
    expect(backend.session.dialog).toEqual(question);
  });
});
