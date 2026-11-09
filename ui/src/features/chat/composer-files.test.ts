import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ipc } from '$lib/ipc/client';
import type { ProjectFile } from '$lib/ipc/bindings/ProjectFile';

import type { AgentAction } from '$lib/ipc/bindings/AgentAction';
import type { AgentEvent } from '$lib/ipc/bindings/AgentEvent';
import type { CommandName } from '$lib/ipc/commands';

import { chatSession, PROJECT, STREAMING } from '../../../tests/chat-helpers';
import { startMockApp } from '../../../tests/mock-app';
import Composer from './Composer.svelte';

function mount(streaming = false, events: AgentEvent[] = streaming ? STREAMING : [{ type: 'ready', protocolVersion: 1 }], answer?: (action: AgentAction) => unknown) {
  const backend = chatSession([...events]);
  const invoke = backend.transport.invoke;

  if (answer) {
    backend.transport.invoke = ((name: CommandName, args: { action?: AgentAction }) => {
      const reply = name === 'agent_send' && args.action ? answer(args.action) : undefined;

      return reply === undefined ? invoke(name, args as never) : reply;
    }) as typeof invoke;
  }

  render(Composer, { session: backend.session });

  const box = screen.getByRole('combobox', { name: 'Message' }) as HTMLTextAreaElement;

  return { ...backend, box };
}

/** Types `text` the way a user does: the caret ends up after it (or at `caret`) and the input event fires. */
async function type(box: HTMLTextAreaElement, text: string, caret = text.length) {
  box.value = text;
  box.setSelectionRange(caret, caret);
  await fireEvent.input(box);
}

const prompts = (of: ReturnType<typeof chatSession>['of']) => of('agent_send').filter((call) => call.action.type === 'prompt');
const send = () => screen.getByRole('button', { name: 'Send' });
const names = () => screen.queryAllByRole('option').map((option) => option.textContent?.replace(/\s+/g, ' ').trim());
const menu = () => screen.queryByRole('listbox', { name: 'Project files' });

/** The project's `project_files` calls so far, and a way to answer them one by one. */
function holdReplies() {
  const pending: { query: string; reply: (files: ProjectFile[]) => void }[] = [];

  vi.mocked(ipc.invoke).mockImplementation(((name: string, args: { query?: string }) => {
    if (name !== 'project_files') {
      return real(name as never, args as never);
    }

    return new Promise((resolve) => pending.push({ query: args.query ?? '', reply: resolve as (files: ProjectFile[]) => void }));
  }) as typeof ipc.invoke);

  return pending;
}

function projectFileCalls() {
  return vi.mocked(ipc.invoke).mock.calls.filter((call) => call[0] === 'project_files');
}

const file = (path: string, isDir = false, changed = false): ProjectFile => ({ path, isDir, changed });
const pause = (ms = 200) => new Promise((resolve) => setTimeout(resolve, ms));

let real: typeof ipc.invoke;

beforeEach(async () => {
  await startMockApp(PROJECT);
  real = ipc.invoke.bind(ipc);
  vi.spyOn(ipc, 'invoke');
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('the @ menu', () => {
  it('lists project files after an @ and narrows them as you type', async () => {
    const { box } = mount();

    await type(box, '@');
    expect(await screen.findByRole('listbox', { name: 'Project files' })).toBeInTheDocument();
    expect(screen.getAllByRole('option').length).toBeGreaterThan(5);

    await type(box, '@main');
    await waitFor(() => expect(names()).toEqual(['main.rs src']));
  });

  it('shows the folder of a file apart from its name, and a Changed badge for changed files only', async () => {
    const { box } = mount();

    await type(box, '@src/');
    await waitFor(() => expect(screen.getAllByRole('option').length).toBeGreaterThan(1));

    const changed = names().filter((row) => row?.includes('Changed'));
    expect(changed.length).toBeGreaterThan(0);
    expect(changed.length).toBeLessThan(names().length);
  });

  it('puts the plain path in the draft on Enter and does not send the message', async () => {
    const { box, session, of } = mount();

    await type(box, 'read @main');
    await screen.findByRole('listbox', { name: 'Project files' });
    await fireEvent.keyDown(box, { key: 'Enter' });

    await waitFor(() => expect(session.draft).toBe('read src/main.rs '));
    expect(of('agent_send').filter((call) => call.action.type === 'prompt')).toEqual([]);
    expect(menu()).toBeNull();
  });

  it('accepts on Tab and puts the caret after the path and its space', async () => {
    const { box, session } = mount();

    await type(box, '@main then');
    box.setSelectionRange(5, 5);
    await fireEvent.keyUp(box, { key: 'ArrowLeft' });
    await screen.findByRole('listbox', { name: 'Project files' });
    await fireEvent.keyDown(box, { key: 'Tab' });

    await waitFor(() => expect(session.draft).toBe('src/main.rs then'));
    expect(box.selectionStart).toBe('src/main.rs'.length);
    expect(box.selectionEnd).toBe('src/main.rs'.length);
  });

  it('gives a picked folder a trailing slash', async () => {
    const { box, session } = mount();

    await type(box, '@docs');
    await waitFor(() => expect(names()).toEqual(['guide.md docs Changed', 'Folder docs']));
    await fireEvent.keyDown(box, { key: 'ArrowDown' });
    await fireEvent.keyDown(box, { key: 'Enter' });

    await waitFor(() => expect(session.draft).toBe('docs/ '));
    expect(box.selectionStart).toBe(6);
  });

  it('accepts a row on click and keeps the focus in the composer', async () => {
    const { box, session } = mount();
    box.focus();

    await type(box, '@main');
    await screen.findByRole('listbox', { name: 'Project files' });
    const row = screen.getAllByRole('option')[0];
    await fireEvent.mouseDown(row);
    await fireEvent.click(row);

    await waitFor(() => expect(session.draft).toBe('src/main.rs '));
    expect(document.activeElement).toBe(box);
  });

  it('wraps the active row around with the arrows, and aria-activedescendant follows it', async () => {
    const { box } = mount();

    await type(box, '@');
    await screen.findByRole('listbox', { name: 'Project files' });

    const options = screen.getAllByRole('option');
    expect(box).toHaveAttribute('aria-expanded', 'true');
    expect(box).toHaveAttribute('aria-activedescendant', options[0].id);
    expect(box.getAttribute('aria-controls')).toBe(menu()?.id);

    await fireEvent.keyDown(box, { key: 'ArrowDown' });
    expect(box).toHaveAttribute('aria-activedescendant', options[1].id);
    expect(options[1]).toHaveAttribute('aria-selected', 'true');

    await fireEvent.keyDown(box, { key: 'ArrowUp' });
    await fireEvent.keyDown(box, { key: 'ArrowUp' });
    expect(box).toHaveAttribute('aria-activedescendant', options.at(-1)?.id);

    await fireEvent.keyDown(box, { key: 'ArrowDown' });
    expect(box).toHaveAttribute('aria-activedescendant', options[0].id);
  });

  it('closes on Escape without being the first Escape of the stop, and stays closed until the draft changes', async () => {
    const { box, session, of } = mount(true);

    await type(box, 'look at @main');
    await screen.findByRole('listbox', { name: 'Project files' });

    await fireEvent.keyDown(box, { key: 'Escape' });
    expect(menu()).toBeNull();
    expect(screen.queryByText('Esc again to stop')).toBeNull();
    expect(session.draft).toBe('look at @main');

    await fireEvent.keyUp(box, { key: 'Escape' });
    await fireEvent.click(box);
    await pause();
    expect(menu()).toBeNull();
    expect(box).toHaveAttribute('aria-expanded', 'false');

    await fireEvent.keyDown(box, { key: 'Escape' });
    await fireEvent.keyDown(box, { key: 'Escape' });
    await waitFor(() => expect(of('agent_send').some((call) => call.action.type === 'abort')).toBe(true));

    await type(box, 'look at @mai');
    expect(await screen.findByRole('listbox', { name: 'Project files' })).toBeInTheDocument();
  });

  it('ignores a slow reply to an older query', async () => {
    const pending = holdReplies();
    const { box } = mount();

    await type(box, '@s');
    await waitFor(() => expect(pending).toHaveLength(1));
    await type(box, '@sr');
    await waitFor(() => expect(pending).toHaveLength(2));

    pending[1].reply([file('src/newer.rs')]);
    await waitFor(() => expect(names()).toEqual(['newer.rs src']));

    pending[0].reply([file('older.rs')]);
    await pause();
    expect(names()).toEqual(['newer.rs src']);
  });

  it('keeps the rows on screen until the next reply arrives', async () => {
    const pending = holdReplies();
    const { box } = mount();

    await type(box, '@s');
    await waitFor(() => expect(pending).toHaveLength(1));
    pending[0].reply([file('src/first.rs')]);
    await waitFor(() => expect(names()).toEqual(['first.rs src']));

    await type(box, '@sr');
    await waitFor(() => expect(pending).toHaveLength(2));
    expect(names()).toEqual(['first.rs src']);

    pending[1].reply([file('src/second.rs')]);
    await waitFor(() => expect(names()).toEqual(['second.rs src']));
  });

  it('closes when nothing matches', async () => {
    const { box } = mount();

    await type(box, '@main');
    await screen.findByRole('listbox', { name: 'Project files' });

    await type(box, '@zzzzqq');
    await waitFor(() => expect(menu()).toBeNull());
  });

  it('closes quietly when the project cannot be read', async () => {
    vi.mocked(ipc.invoke).mockImplementation(((name: string, args: object) => (name === 'project_files' ? Promise.reject(new Error('gone')) : real(name as never, args as never))) as typeof ipc.invoke);
    const { box, notifications } = mount();

    await type(box, '@main');
    await waitFor(() => expect(projectFileCalls()).toHaveLength(1));
    await pause();

    expect(menu()).toBeNull();
    expect(JSON.stringify(notifications.items)).not.toContain('gone');
  });

  it('asks the chat’s own project', async () => {
    const { box } = mount();

    await type(box, '@main');
    await waitFor(() => expect(projectFileCalls()).toHaveLength(1));

    expect(projectFileCalls()[0][1]).toEqual({ project: PROJECT, query: 'main', limit: 20 });
  });

  it('does not open for text the program put in the draft, only for what the user types next', async () => {
    const { box, session } = mount();

    session.draft = 'check @src/main.rs';
    await pause();
    expect(projectFileCalls()).toEqual([]);
    expect(menu()).toBeNull();

    session.ingest({ sessionKey: session.key, seq: 9, event: { type: 'editorText', text: 'again @main' } });
    session.flush();
    await waitFor(() => expect(session.draft).toBe('again @main'));
    await pause();
    expect(projectFileCalls()).toEqual([]);

    await type(box, 'again @mai');
    expect(await screen.findByRole('listbox', { name: 'Project files' })).toBeInTheDocument();
  });

  it.each([['!ls @main'], ['mail a@main.com'], ['`code @main'], ['```\n@main']])('does not open in %j', async (text) => {
    const { box } = mount();

    await type(box, text);
    await pause();

    expect(projectFileCalls()).toEqual([]);
    expect(menu()).toBeNull();
  });

  it('opens in the arguments of a slash command', async () => {
    const { box } = mount();

    await type(box, '/review @main');

    expect(await screen.findByRole('listbox', { name: 'Project files' })).toBeInTheDocument();
  });

  it('leaves the slash menu as it was', async () => {
    const { box, session } = mount();

    await type(box, '/revi');
    expect(await screen.findByRole('listbox', { name: 'Slash commands' })).toBeInTheDocument();
    expect(box).toHaveAttribute('aria-activedescendant', screen.getAllByRole('option')[0].id);

    await fireEvent.keyDown(box, { key: 'Tab' });

    expect(session.draft).toBe('/review ');
    await pause();
    expect(projectFileCalls()).toEqual([]);
  });

  it('does not open once the chat has ended', async () => {
    const { box, session } = mount();

    await type(box, '@main');
    await screen.findByRole('listbox', { name: 'Project files' });

    session.ingest({ sessionKey: session.key, seq: 9, event: { type: 'exited', code: 0, signal: null } });
    session.flush();

    await waitFor(() => expect(menu()).toBeNull());
  });

  describe('a draft the user did not type', () => {
    it('does not open after a rejected message comes back to the composer', async () => {
      const { box, session } = mount(false, undefined, (action) => (action.type === 'prompt' ? Promise.reject(new Error('refused')) : undefined));

      await type(box, 'see @src');
      await waitFor(() => expect(projectFileCalls()).toHaveLength(1));
      await fireEvent.click(send());
      await waitFor(() => expect(session.draft).toBe('see @src'));
      await pause();

      expect(projectFileCalls()).toHaveLength(1);
      expect(menu()).toBeNull();

      await type(box, 'see @sr');
      expect(await screen.findByRole('listbox', { name: 'Project files' })).toBeInTheDocument();
    });

    it('does not open for the same words put back with Use again', async () => {
      const { box, session, of } = mount();

      await type(box, 'see @src');
      await waitFor(() => expect(projectFileCalls()).toHaveLength(1));
      await fireEvent.click(send());
      await waitFor(() => expect(prompts(of)).toHaveLength(1));
      await waitFor(() => expect(session.draft).toBe(''));

      session.useAgain('see @src');
      await waitFor(() => expect(session.draft).toBe('see @src'));
      await pause();

      expect(projectFileCalls()).toHaveLength(1);
      expect(menu()).toBeNull();
    });

    it('does not open for the same words taken back from the queue', async () => {
      const queue: AgentEvent = { type: 'queueUpdate', steering: ['see @src'], followUp: [], liveSteered: 0 };
      const { box, session } = mount(true, [...STREAMING, queue], (action) => (action.type === 'removeQueued' ? Promise.resolve({ type: 'queueRemoval', removed: true }) : undefined));

      await type(box, 'see @src');
      await waitFor(() => expect(projectFileCalls()).toHaveLength(1));
      await fireEvent.click(send());
      await waitFor(() => expect(session.draft).toBe(''));

      await fireEvent.keyDown(box, { key: 'ArrowUp', altKey: true });
      await waitFor(() => expect(session.draft).toBe('see @src'));
      await pause();

      expect(projectFileCalls()).toHaveLength(1);
      expect(menu()).toBeNull();
    });
  });

  describe('a choice made while the rows answer an older query', () => {
    /** The menu showing `@s`, and the box about to become `@sr`. */
    async function staleMenu() {
      const pending = holdReplies();
      const mounted = mount();

      await type(mounted.box, '@s');
      await waitFor(() => expect(pending).toHaveLength(1));
      pending[0].reply([file('src/first.rs')]);
      await waitFor(() => expect(names()).toEqual(['first.rs src']));

      await type(mounted.box, '@sr');
      await waitFor(() => expect(pending).toHaveLength(2));

      return { ...mounted, pending };
    }

    it.each(['Enter', 'Tab'])('waits on %s for the reply to the current query and takes its first row', async (key) => {
      const { box, session, of, pending } = await staleMenu();

      await fireEvent.keyDown(box, { key });
      expect(session.draft).toBe('@sr');
      expect(names()).toEqual(['first.rs src']);

      pending[1].reply([file('src/second.rs'), file('src/third.rs')]);

      await waitFor(() => expect(session.draft).toBe('src/second.rs '));
      expect(prompts(of)).toEqual([]);
    });

    it('inserts and sends nothing when the reply is empty, and the next Enter sends', async () => {
      const { box, session, of, pending } = await staleMenu();

      await fireEvent.keyDown(box, { key: 'Enter' });
      pending[1].reply([]);

      await waitFor(() => expect(menu()).toBeNull());
      expect(session.draft).toBe('@sr');
      expect(prompts(of)).toEqual([]);

      await fireEvent.keyDown(box, { key: 'Enter' });
      await waitFor(() => expect(prompts(of)).toHaveLength(1));
    });

    it('still picks a visible row on click', async () => {
      const { box, session } = await staleMenu();
      box.focus();

      const row = screen.getAllByRole('option')[0];
      await fireEvent.mouseDown(row);
      await fireEvent.click(row);

      await waitFor(() => expect(session.draft).toBe('src/first.rs '));
    });

    it('is forgotten when the user types on', async () => {
      const { box, session, pending } = await staleMenu();

      await fireEvent.keyDown(box, { key: 'Enter' });
      await type(box, '@src');
      await waitFor(() => expect(pending).toHaveLength(3));

      pending[1].reply([file('src/second.rs')]);
      pending[2].reply([file('src/third.rs')]);
      await waitFor(() => expect(names()).toEqual(['third.rs src']));

      expect(session.draft).toBe('@src');
    });

    it('is forgotten when the caret moves to another mention of the same query', async () => {
      const pending = holdReplies();
      const { box, session } = mount();

      await type(box, '@s and @s');
      await waitFor(() => expect(pending).toHaveLength(1));
      pending[0].reply([file('src/first.rs')]);
      await waitFor(() => expect(names()).toEqual(['first.rs src']));

      await type(box, '@sr and @sr');
      await waitFor(() => expect(pending).toHaveLength(2));
      await fireEvent.keyDown(box, { key: 'Enter' });

      box.setSelectionRange(3, 3);
      await fireEvent.keyUp(box, { key: 'ArrowLeft' });
      pending.at(-1)?.reply([file('src/second.rs')]);

      await waitFor(() => expect(names()).toEqual(['second.rs src']));
      expect(session.draft).toBe('@sr and @sr');
    });

    it('is forgotten on Escape', async () => {
      const { box, session, pending } = await staleMenu();

      await fireEvent.keyDown(box, { key: 'Escape' });
      pending[1].reply([file('src/second.rs')]);
      await pause();

      expect(session.draft).toBe('@sr');
    });

    it('is not met by a reply to an earlier query', async () => {
      const { box, session, pending } = await staleMenu();

      await fireEvent.keyDown(box, { key: 'Enter' });
      await type(box, '@s');
      await waitFor(() => expect(pending).toHaveLength(3));
      await type(box, '@sr');
      await waitFor(() => expect(pending).toHaveLength(4));

      pending[1].reply([file('src/stale.rs')]);
      pending[3].reply([file('src/second.rs')]);
      await waitFor(() => expect(names()).toEqual(['second.rs src']));

      expect(session.draft).toBe('@sr');
    });
  });
});
