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

    await waitFor(() => expect(sentActions(of)).toEqual([{ type: 'prompt', message: 'hello', images: [], streamingBehavior: null }]));
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
        { type: 'steer', message: 'change course', images: [] },
        { type: 'steer', message: 'again', images: [] },
      ]),
    );
  });

  it('queues a follow-up on Ctrl+Enter while the agent streams', async () => {
    const { box, of } = mount(true);

    await type(box, 'and then tests');
    await fireEvent.keyDown(box, { key: 'Enter', ctrlKey: true });

    await waitFor(() => expect(sentActions(of)).toEqual([{ type: 'followUp', message: 'and then tests', images: [] }]));
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

describe('Composer images', () => {
  const png = () => new File(['png-bytes'], 'shot.png', { type: 'image/png' });

  it('shows an attached image, sends it with the message and clears it', async () => {
    const { box, of, session } = mount();
    const input = document.querySelector<HTMLInputElement>('input[type="file"]')!;

    await fireEvent.change(input, { target: { files: [png()] } });
    await screen.findByRole('img', { name: 'shot.png' });
    expect(session.attached.images).toHaveLength(1);

    await type(box, 'what is this?');
    await fireEvent.keyDown(box, { key: 'Enter' });

    await waitFor(() =>
      expect(sentActions(of)).toEqual([{ type: 'prompt', message: 'what is this?', images: [{ data: btoa('png-bytes'), mimeType: 'image/png' }], streamingBehavior: null }]),
    );
    expect(session.attached.images).toEqual([]);
    expect(screen.queryByRole('img', { name: 'shot.png' })).toBeNull();
  });

  it('removes an attachment on request', async () => {
    const { session } = mount();
    const input = document.querySelector<HTMLInputElement>('input[type="file"]')!;

    await fireEvent.change(input, { target: { files: [png()] } });
    await fireEvent.click(await screen.findByRole('button', { name: 'Remove shot.png' }));

    expect(session.attached.images).toEqual([]);
  });

  it('saves any other file and sends its path with the message', async () => {
    const { box, of, session } = mount();
    const input = document.querySelector<HTMLInputElement>('input[type="file"]')!;

    await fireEvent.change(input, { target: { files: [new File(['error: boom'], 'build.log', { type: 'text/plain' })] } });
    await screen.findByText('build.log');
    expect(of('attachment_save')).toEqual([{ name: 'build.log', data: btoa('error: boom') }]);
    expect(session.attached.images).toEqual([]);

    await type(box, 'why does it fail?');
    await fireEvent.keyDown(box, { key: 'Enter' });

    await waitFor(() =>
      expect(sentActions(of)).toEqual([
        {
          type: 'prompt',
          message: 'why does it fail?\n\nAttached files (open them with your file tools):\n- /home/demo/.cache/attachments/mock/build.log (11 B)',
          images: [],
          streamingBehavior: null,
        },
      ]),
    );
    expect(session.attached.files).toEqual([]);
  });

  it('sends a file on its own, without text', async () => {
    const { box, of } = mount();
    const input = document.querySelector<HTMLInputElement>('input[type="file"]')!;

    await fireEvent.change(input, { target: { files: [new File(['x'], 'a.json', { type: 'application/json' })] } });
    await screen.findByText('a.json');
    await fireEvent.keyDown(box, { key: 'Enter' });

    await waitFor(() => expect(sentActions(of)).toHaveLength(1));
  });

  it('turns a large paste into a text file and leaves a short one alone', async () => {
    const { box, of, session } = mount();
    const paste = (text: string) => fireEvent.paste(box, { clipboardData: { files: [], getData: () => text } });

    await paste('short');
    expect(of('attachment_save')).toEqual([]);

    await paste(Array.from({ length: 60 }, (_, line) => `log line ${line}`).join('\n'));
    await waitFor(() => expect(session.attached.files).toHaveLength(1));
    expect(session.attached.files[0].name).toMatch(/^pasted-text-\d{8}-\d{6}\.txt$/);
  });

  it('lists files a native drop only named', async () => {
    const { of, session } = mount();
    const [{ onDrop }] = of('drop_listen');

    onDrop({ type: 'dropped', images: [], files: [{ name: 'huge.log', path: '/var/log/huge.log', bytes: 90_000_000 }], skipped: [] });
    onDrop({ type: 'dropped', images: [], files: [{ name: 'huge.log', path: '/var/log/huge.log', bytes: 90_000_000 }], skipped: [] });

    await screen.findByText('huge.log');
    expect(session.attached.files).toHaveLength(1);
  });

  it('previews images dragged in from the file manager, which the shell reads', async () => {
    const { of, session, notifications } = mount();
    const [{ onDrop }] = of('drop_listen');

    onDrop({ type: 'hover', active: true });
    onDrop({ type: 'dropped', images: [{ name: 'photo.jpeg', mimeType: 'image/jpeg', data: btoa('jpeg-bytes') }], files: [], skipped: ['notes'] });

    await screen.findByRole('img', { name: 'photo.jpeg' });
    expect(session.attached.images.map((attachment) => attachment.mimeType)).toEqual(['image/jpeg']);
    expect(notifications.items.map((toast) => toast.title)).toContain('Could not attach notes');

    await fireEvent.click(screen.getByRole('button', { name: 'Remove photo.jpeg' }));
    expect(session.attached.images).toEqual([]);
  });
});
