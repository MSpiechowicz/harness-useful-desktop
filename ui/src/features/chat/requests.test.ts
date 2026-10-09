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
