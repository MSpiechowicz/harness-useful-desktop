import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { AgentSessionInfo } from '$lib/ipc/bindings/AgentSessionInfo';

import { chatSession, startedSession, STREAMING } from '../../../tests/chat-helpers';
import BashConfirmDialog from './BashConfirmDialog.svelte';
import Composer from './Composer.svelte';

function mount(streaming = false, info: Partial<AgentSessionInfo> = {}) {
  const backend = chatSession(streaming ? STREAMING : [{ type: 'ready', protocolVersion: 1 }], info);
  render(Composer, { session: backend.session });

  const box = screen.getByRole('combobox', { name: 'Message' }) as HTMLTextAreaElement;

  return { ...backend, box };
}

async function type(box: HTMLTextAreaElement, text: string) {
  await fireEvent.input(box, { target: { value: text } });
}

function sentActions(of: ReturnType<typeof chatSession>['of']) {
  return of('agent_send')
    .map((call) => call.action)
    .filter((action) => !['refreshCommands', 'getState'].includes(action.type));
}

describe('Composer keys', () => {
  it('sends on Enter and clears the draft', async () => {
    const { box, of, session } = mount();

    await type(box, 'hello');
    await fireEvent.keyDown(box, { key: 'Enter' });

    await waitFor(() => expect(sentActions(of)).toEqual([{ type: 'prompt', message: 'hello', streamingBehavior: null }]));
    expect(session.draft).toBe('');
  });

  it('does not send on Shift+Enter', async () => {
    const { box, of, session } = mount();

    await type(box, 'line one');
    await fireEvent.keyDown(box, { key: 'Enter', shiftKey: true });

    expect(sentActions(of)).toEqual([]);
    expect(session.draft).toBe('line one');
  });

  it('ignores Enter on an empty draft', async () => {
    const { box, of } = mount();

    await fireEvent.keyDown(box, { key: 'Enter' });

    expect(sentActions(of)).toEqual([]);
  });

  it('steers on Enter and Alt+Enter while the agent streams', async () => {
    const { box, of } = mount(true);

    await type(box, 'change course');
    await fireEvent.keyDown(box, { key: 'Enter' });
    await type(box, 'again');
    await fireEvent.keyDown(box, { key: 'Enter', altKey: true });

    await waitFor(() =>
      expect(sentActions(of)).toEqual([
        { type: 'steer', message: 'change course' },
        { type: 'steer', message: 'again' },
      ]),
    );
  });

  it('queues a follow-up on Ctrl+Enter while the agent streams', async () => {
    const { box, of } = mount(true);

    await type(box, 'and then tests');
    await fireEvent.keyDown(box, { key: 'Enter', ctrlKey: true });

    await waitFor(() => expect(sentActions(of)).toEqual([{ type: 'followUp', message: 'and then tests' }]));
  });

  it('turns the send button into Stop, Follow-up and Steer while streaming', async () => {
    const { of } = mount(true);

    await fireEvent.click(screen.getByRole('button', { name: /Stop/ }));

    await waitFor(() => expect(sentActions(of)).toEqual([{ type: 'abort' }]));
    expect(screen.getByRole('button', { name: /Follow-up/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Steer/ })).toBeInTheDocument();
  });

  it('puts text the agent asks for into the composer', async () => {
    const backend = chatSession([{ type: 'ready', protocolVersion: 1 }]);
    render(Composer, { session: backend.session });

    backend.session.ingest({ sessionKey: backend.session.key, seq: 2, event: { type: 'editorText', text: '/review src/lib.rs' } });
    backend.session.flush();

    await waitFor(() => expect(backend.session.draft).toBe('/review src/lib.rs'));
  });
});

describe('slash commands', () => {
  it('suggests commands as you type and narrows the list', async () => {
    const { box } = mount();

    await type(box, '/');
    expect((await screen.findAllByRole('option')).length).toBeGreaterThan(3);

    await type(box, '/compa');
    await waitFor(() => expect(screen.getAllByRole('option')[0]).toHaveTextContent('/compact'));
    expect(screen.getAllByRole('option')[0]).toHaveTextContent('builtin');
    expect(screen.getAllByRole('option').length).toBeLessThan(5);
  });

  it('inserts a prompt-text command and leaves the sending to the user', async () => {
    const { box, session, of } = mount();

    await type(box, '/revi');
    await fireEvent.keyDown(box, { key: 'Tab' });

    expect(session.draft).toBe('/review ');
    expect(sentActions(of)).toEqual([]);
  });

  it('runs an rpc command at once and clears the draft', async () => {
    const { box, session, of } = mount(false, { harness: 'pi' });

    await type(box, '/new');
    await waitFor(() => expect(screen.getAllByRole('option')[0]).toHaveTextContent('/new'));
    await fireEvent.keyDown(box, { key: 'Enter' });

    await waitFor(() => expect(sentActions(of)).toEqual([{ type: 'newSession' }]));
    expect(session.draft).toBe('');
  });

  it('asks for instructions before compacting from the menu, and sends typed arguments directly', async () => {
    const { box, session, of } = mount(false, { harness: 'pi' });

    await type(box, '/compa');
    await fireEvent.keyDown(box, { key: 'Enter' });
    expect(session.ui.ask).toBe('compact');

    session.ui.ask = null;
    await type(box, '/compact keep the notes ');
    await fireEvent.keyDown(box, { key: 'Enter' });

    await waitFor(() => expect(sentActions(of)).toEqual([{ type: 'compact', instructions: 'keep the notes' }]));
  });

  it('opens a picker for a gui command', async () => {
    const { box, session } = mount();

    await type(box, '/model');
    await waitFor(() => expect(screen.getAllByRole('option')[0]).toHaveTextContent('/model'));
    await fireEvent.keyDown(box, { key: 'Enter' });

    expect(session.ui.modelPicker).toBe(true);
  });

  it('closes the suggestions on Escape without touching the draft', async () => {
    const { box, session } = mount();

    await type(box, '/co');
    await screen.findAllByRole('option');
    await fireEvent.keyDown(box, { key: 'Escape' });

    expect(screen.queryByRole('listbox')).toBeNull();
    expect(session.draft).toBe('/co');
  });
});

describe('shell commands', () => {
  it('never runs without confirmation', async () => {
    const { box, session, of } = mount();

    await type(box, '!rm -rf build');
    await fireEvent.keyDown(box, { key: 'Enter' });

    expect(session.ui.confirmBash).toBe('rm -rf build');
    expect(of('agent_bash')).toEqual([]);
    expect(session.draft).toBe('!rm -rf build');
  });

  it('shows the exact command and runs it only after the user confirms', async () => {
    const { session, of } = await startedSession();
    render(BashConfirmDialog, { session, command: 'git status --short' });

    expect(await screen.findByRole('dialog', { name: 'Run this command?' })).toHaveTextContent('git status --short');
    expect(of('agent_bash')).toEqual([]);

    await fireEvent.click(screen.getByRole('button', { name: 'Run command' }));

    await waitFor(() => expect(of('agent_bash')).toEqual([{ key: session.key, command: 'git status --short', confirmed: true }]));
    await waitFor(() => expect(session.chat.items.at(-1)).toMatchObject({ kind: 'bash', running: false, exitCode: 0 }));
  });

  it('drops the command when the user cancels', async () => {
    const { session, of } = chatSession();
    session.ui.confirmBash = 'ls';
    render(BashConfirmDialog, { session, command: 'ls' });

    await fireEvent.click(await screen.findByRole('button', { name: 'Cancel' }));

    expect(session.ui.confirmBash).toBeNull();
    expect(of('agent_bash')).toEqual([]);
  });
});
