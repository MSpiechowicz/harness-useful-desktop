import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { AskQuestion } from '$lib/ipc/bindings/AskQuestion';
import type { UiRequest } from '$lib/ipc/bindings/UiRequest';

import { startedSession } from '../../../tests/chat-helpers';
import UiRequestHost from './UiRequestHost.svelte';

const request: UiRequest = {
  id: 'ask-1',
  purpose: 'question',
  kind: {
    type: 'questions',
    timeoutMs: null,
    questions: [
      { id: 'area', question: 'Which area?', multi: false, header: null, custom: true, input: 'text', recommended: 1, options: [{ label: 'Mark', description: 'The H' }, { label: 'Motion', description: null }] },
      { id: 'how', question: 'How far?', multi: true, header: null, custom: true, input: 'text', recommended: null, options: [{ label: 'Small (Recommended)', description: null }, { label: 'Large', description: null }] },
    ],
  },
};

const only = (question: AskQuestion): UiRequest => ({
  id: 'ask-2',
  purpose: 'question',
  kind: { type: 'questions', timeoutMs: null, questions: [question] },
});

async function open(asked: UiRequest = request) {
  const backend = await startedSession();

  render(UiRequestHost, { session: backend.session });
  backend.session.ingest({ sessionKey: backend.session.key, seq: 1, event: { type: 'uiRequest', request: asked } });
  backend.session.flush();

  return { ...backend, sent: () => backend.of('agent_ui_respond').map((call) => call.response) };
}

const click = (name: RegExp | string) => fireEvent.click(screen.getByRole('button', { name }));

describe('an ask of several questions', () => {
  afterEach(cleanup);

  it('goes back keeping the answer, sends nothing before the end and sends every answer together', async () => {
    const { sent } = await open();

    await screen.findByRole('dialog', { name: 'Which area?' });
    await click(/Mark/);
    await click('Next');
    await screen.findByRole('dialog', { name: 'How far?' });
    expect(screen.getByText('Question 2 of 2')).toBeInTheDocument();

    await click('Back');
    await screen.findByRole('dialog', { name: 'Which area?' });
    expect(screen.getByRole('button', { name: /Mark/ })).toHaveAttribute('aria-pressed', 'true');

    await click(/Motion/);
    expect(screen.getByRole('dialog', { name: 'Which area?' })).toBeInTheDocument();
    await click('Next');
    await screen.findByRole('dialog', { name: 'How far?' });
    await click(/Small/);
    await click(/Large/);
    expect(sent()).toEqual([]);

    await click('Submit');

    await waitFor(() =>
      expect(sent()).toEqual([
        {
          id: 'ask-1',
          outcome: {
            type: 'answers',
            answers: [
              { id: 'area', selectedOptions: ['Motion'], customInput: null },
              { id: 'how', selectedOptions: ['Small (Recommended)', 'Large'], customInput: null },
            ],
          },
        },
      ]),
    );
  });

  it('sends a typed answer instead of an option', async () => {
    const { sent } = await open();

    await screen.findByRole('dialog', { name: 'Which area?' });
    await click(/Mark/);
    await click('Next');
    await screen.findByRole('dialog', { name: 'How far?' });
    await click('Back');
    await screen.findByRole('dialog', { name: 'Which area?' });
    await click(/Other/);
    await fireEvent.input(screen.getByRole('textbox'), { target: { value: ' legibility ' } });
    await click('Next');
    await screen.findByRole('dialog', { name: 'How far?' });
    await click(/Large/);
    await click('Submit');

    await waitFor(() => expect(sent()[0].outcome).toMatchObject({ answers: [{ id: 'area', selectedOptions: [], customInput: 'legibility' }, { id: 'how' }] }));
  });

  it('marks the recommended option, from the agent or its label, and cannot be left without an answer', async () => {
    await open();

    const dialog = await screen.findByRole('dialog', { name: 'Which area?' });
    expect(screen.getByRole('button', { name: /Motion/ })).toHaveTextContent('Recommended');
    expect(screen.getByRole('button', { name: /Mark/ })).not.toHaveTextContent('Recommended');

    await click(/Mark/);
    await click('Next');
    await screen.findByRole('dialog', { name: 'How far?' });
    expect(screen.getByRole('button', { name: /Small/ })).toHaveTextContent('Recommended');

    await fireEvent.keyDown(dialog, { key: 'Escape' });
    expect(screen.queryByRole('button', { name: /close/i })).toBeNull();
    expect(screen.getByRole('dialog')).toBe(dialog);
  });

  it('needs an answer to a single choice, and keeps the badge but not the border when another option is picked', async () => {
    await open(only({ id: '0', header: null, question: 'Which?', multi: false, custom: false, input: 'text', recommended: 0, options: [{ label: 'A', description: null }, { label: 'B', description: null }] }));

    await screen.findByRole('dialog', { name: 'Which?' });
    expect(screen.getByRole('button', { name: 'Submit' })).toBeDisabled();
    expect(screen.queryByRole('button', { name: /Other/ })).toBeNull();
    expect(screen.getByRole('button', { name: /A/ }).className).toContain('border-accent/50');

    await click(/^2/);

    expect(screen.getByRole('button', { name: 'Submit' })).toBeEnabled();
    expect(screen.getByRole('button', { name: /^1/ })).toHaveTextContent('Recommended');
    expect(screen.getByRole('button', { name: /^1/ }).className).not.toContain('border-accent');
  });

  it('drops the recommended border as soon as Other is clicked, before anything is typed', async () => {
    await open(only({ id: '0', header: null, question: 'Which?', multi: true, custom: true, input: 'text', recommended: 0, options: [{ label: 'A', description: null }] }));

    await screen.findByRole('dialog', { name: 'Which?' });
    expect(screen.getByText('Select all that apply')).toBeInTheDocument();
    await click(/Other/);

    expect(screen.getByRole('button', { name: /^1/ })).toHaveTextContent('Recommended');
    expect(screen.getByRole('button', { name: /^1/ }).className).not.toContain('border-accent');
  });

  it('asks a question without options as typed text and holds back a bad number', async () => {
    const { sent } = await open(only({ id: '0', header: null, question: 'How many?', multi: false, custom: true, input: 'integer', recommended: null, options: [] }));

    await screen.findByRole('dialog', { name: 'How many?' });
    const field = screen.getByRole('spinbutton');

    await fireEvent.input(field, { target: { value: '1.5' } });
    expect(screen.getByRole('button', { name: 'Submit' })).toBeDisabled();

    await fireEvent.input(field, { target: { value: '1e3' } });
    expect(screen.getByRole('button', { name: 'Submit' })).toBeDisabled();

    await fireEvent.input(field, { target: { value: '3' } });
    await click('Submit');

    await waitFor(() => expect(sent()[0].outcome).toEqual({ type: 'answers', answers: [{ id: '0', selectedOptions: [], customInput: '3' }] }));
  });

  it('comes back when the answer is refused, because the agent is still waiting', async () => {
    const backend = await open();
    const transport = (backend.session as unknown as { transport: { invoke: (...args: unknown[]) => Promise<unknown> } }).transport;

    await screen.findByRole('dialog', { name: 'Which area?' });
    await click(/Mark/);
    await click('Next');
    await screen.findByRole('dialog', { name: 'How far?' });
    await click(/Large/);

    vi.spyOn(transport, 'invoke').mockRejectedValueOnce({ code: 'invalidInput', message: 'refused' });
    await click('Submit');

    await waitFor(() => expect(backend.session.dialog?.id).toBe('ask-1'));
    expect(await screen.findByRole('dialog', { name: 'How far?' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Large/ })).toHaveAttribute('aria-pressed', 'true');

    await click('Back');
    await screen.findByRole('dialog', { name: 'Which area?' });
    expect(screen.getByRole('button', { name: /Mark/ })).toHaveAttribute('aria-pressed', 'true');
  });

  it('sends the answers so far before the deadline, and leaves it to the agent when there are none', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const timed: UiRequest = { ...request, kind: { ...(request.kind as Extract<UiRequest['kind'], { type: 'questions' }>), timeoutMs: 10000 } };
    const { sent } = await open(timed);

    await screen.findByRole('dialog', { name: 'Which area?' });
    await click(/Mark/);
    await vi.advanceTimersByTimeAsync(9000);

    await waitFor(() =>
      expect(sent()[0].outcome).toEqual({
        type: 'answers',
        answers: [
          { id: 'area', selectedOptions: ['Mark'], customInput: null },
          { id: 'how', selectedOptions: [], customInput: null },
        ],
      }),
    );
    vi.useRealTimers();
  });

  it('marks the row picked when two share a label and sends the label once', async () => {
    const { sent } = await open(only({ id: '0', header: null, question: 'Which?', multi: true, custom: true, input: 'text', recommended: null, options: [{ label: 'Yes', description: 'keep' }, { label: 'Yes', description: 'delete' }] }));

    await screen.findByRole('dialog', { name: 'Which?' });
    await click(/^2/);

    expect(screen.getByRole('button', { name: /^1/ })).toHaveAttribute('aria-pressed', 'false');

    await click(/^1/);
    await click('Submit');

    await waitFor(() => expect(sent()[0].outcome).toEqual({ type: 'answers', answers: [{ id: '0', selectedOptions: ['Yes'], customInput: null }] }));
  });

  it('shows the header and reads material after the first line below the title', async () => {
    await open(only({ id: '0', header: 'ratchet · plan', question: 'Ship it?\n\nline one\nline two', multi: false, custom: true, input: 'text', recommended: null, options: [{ label: 'Approve', description: null }] }));

    await screen.findByRole('dialog', { name: 'Ship it?' });
    expect(screen.getByText('ratchet · plan')).toBeInTheDocument();
    expect(document.querySelector('[data-question-body]')?.textContent).toBe('line one\nline two');
  });

  it('leaves out a header that only repeats the question', async () => {
    await open(only({ id: '0', header: 'Lead target', question: 'Which target should lead?', multi: false, custom: true, input: 'text', recommended: null, options: [{ label: 'A', description: null }] }));

    await screen.findByRole('dialog', { name: 'Which target should lead?' });
    expect(screen.queryByText('Lead target')).toBeNull();
  });

  it('opens and moves on with no option focused, so none looks chosen, and the keys still work', async () => {
    await open();

    await screen.findByRole('dialog', { name: 'Which area?' });
    await waitFor(() => expect(document.activeElement).toHaveAttribute('data-focus-start'));

    await fireEvent.keyDown(document.activeElement as Element, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(screen.getByRole('button', { name: /Mark/ }));

    await click(/Mark/);
    await click('Next');
    await screen.findByRole('dialog', { name: 'How far?' });
    await waitFor(() => expect(document.activeElement).toHaveAttribute('data-focus-start'));

    await fireEvent.keyDown(document.activeElement as Element, { key: '2' });
    expect(screen.getByRole('button', { name: /Large/ })).toHaveAttribute('aria-pressed', 'true');
  });

  it('closes with Cancel', async () => {
    const { sent } = await open();

    await screen.findByRole('dialog');
    await click('Cancel');

    await waitFor(() => expect(sent()[0].outcome).toEqual({ type: 'cancelled' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });
});
