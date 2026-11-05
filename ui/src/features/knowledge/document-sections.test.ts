import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { KnowledgeSection } from '$lib/ipc/bindings/KnowledgeSection';
import type { Transport } from '$lib/ipc/transport';

import { ChunkTexts } from './chunk-texts.svelte';
import DocumentSections from './DocumentSections.svelte';
import { sectionsHint } from './knowledge-format';

function section(headingPath: string, chunkId: number): KnowledgeSection {
  return { headingPath, chunkId, ordinal: chunkId, tokens: 10, charStart: 0, charEnd: 10 } as unknown as KnowledgeSection;
}

/** `count` headings of one part each. */
const headings = (count: number): KnowledgeSection[] => Array.from({ length: count }, (_, index) => section(`Heading ${index + 1}`, index + 1));

/** One heading cut into `count` parts. */
const cut = (count: number): KnowledgeSection[] => Array.from({ length: count }, (_, index) => section('Long', index + 1));

function show(sections: KnowledgeSection[]) {
  const invoke = vi.fn((_command: string, args: { chunkId: number }) => Promise.resolve({ chunk: { text: `Text of part ${args.chunkId}` } }));
  const texts = new ChunkTexts(1, { invoke } as unknown as Transport);

  render(DocumentSections, { sections, texts });

  return invoke;
}

describe('a section cut into parts', () => {
  it('has one segment a part up to 24, and any of them jumps to its part', async () => {
    show(cut(24));
    await fireEvent.click(screen.getByRole('button', { name: 'Long' }));

    await fireEvent.click(await screen.findByRole('button', { name: 'Part 7 of 24' }));

    expect(screen.queryByRole('slider')).toBeNull();
    expect(await screen.findByText('Text of part 7')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Part 7 of 24' })).toHaveAttribute('aria-current', 'true');
  });

  it('has one bar from 25, moved by key, and a drag reads only the part it lands on', async () => {
    const invoke = show(cut(100));

    await fireEvent.click(screen.getByRole('button', { name: 'Long' }));

    const bar = await screen.findByRole('slider', { name: 'Parts' });
    const box = { left: 0, width: 116, top: 0, right: 116, bottom: 36, height: 36, x: 0, y: 0, toJSON: () => ({}) };

    bar.getBoundingClientRect = () => box;
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));

    await fireEvent.pointerDown(bar, { clientX: 20, pointerId: 1 });
    await fireEvent.pointerMove(bar, { clientX: 40, pointerId: 1 });
    await fireEvent.pointerMove(bar, { clientX: 60, pointerId: 1 });

    expect(invoke).toHaveBeenCalledTimes(1);
    await fireEvent.pointerUp(bar, { clientX: 60, pointerId: 1 });
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2));

    await fireEvent.keyDown(bar, { key: 'End' });
    expect(await screen.findByText('Text of part 100')).toBeInTheDocument();
    expect(bar).toHaveAttribute('aria-valuenow', '100');
    expect(screen.getByRole('button', { name: 'This is the last part' })).toBeDisabled();
  });

  it('ignores a right click on the bar and leaves browser shortcuts alone', async () => {
    show(cut(100));
    await fireEvent.click(screen.getByRole('button', { name: 'Long' }));

    const bar = await screen.findByRole('slider', { name: 'Parts' });

    await fireEvent.pointerDown(bar, { button: 2, clientX: 60, pointerId: 1 });
    expect(bar).toHaveAttribute('aria-valuenow', '1');

    const untouched = await fireEvent.keyDown(bar, { key: 'ArrowRight', altKey: true });

    expect(untouched).toBe(true);
    expect(bar).toHaveAttribute('aria-valuenow', '1');
  });

  it('moves the keyboard to the other arrow when one reaches the end', async () => {
    show(cut(2));
    await fireEvent.click(screen.getByRole('button', { name: 'Long' }));

    const next = await screen.findByRole('button', { name: 'Next part' });

    next.focus();
    await fireEvent.click(next);

    await waitFor(() => expect(screen.getByRole('button', { name: 'Previous part' })).toHaveFocus());
  });

  it('keeps the text of a part in place until the next part is read', async () => {
    let answer: (value: unknown) => void = () => {};
    const invoke = vi.fn((_command: string, args: { chunkId: number }) => (args.chunkId === 2 ? new Promise((resolve) => (answer = resolve)) : Promise.resolve({ chunk: { text: `Text of part ${args.chunkId}` } })));

    render(DocumentSections, { sections: cut(2), texts: new ChunkTexts(1, { invoke } as unknown as Transport) });
    await fireEvent.click(screen.getByRole('button', { name: 'Long' }));
    expect(await screen.findByText('Text of part 1')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Next part' }));

    expect(screen.getByText('Part 2 of 2')).toBeInTheDocument();
    expect(screen.getByText('Text of part 1')).toBeInTheDocument();
    expect(screen.getByRole('status', { name: 'Reading the text' })).toBeInTheDocument();

    answer({ chunk: { text: 'Text of part 2' } });

    expect(await screen.findByText('Text of part 2')).toBeInTheDocument();
    expect(screen.queryByText('Text of part 1')).toBeNull();
    expect(screen.queryByRole('status', { name: 'Reading the text' })).toBeNull();
  });

  it('reads a part that fails once, and again only when asked', async () => {
    const invoke = vi.fn(() => Promise.reject(new Error('the database is busy')));

    render(DocumentSections, { sections: cut(2), texts: new ChunkTexts(1, { invoke } as unknown as Transport) });
    await fireEvent.click(screen.getByRole('button', { name: 'Long' }));

    const retry = await screen.findByRole('button', { name: 'Try again' });

    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(invoke).toHaveBeenCalledTimes(1);

    await fireEvent.click(retry);
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2));
  });

  it('says that parts overlap only for a document that has parts', () => {
    expect(sectionsHint(headings(3))).not.toMatch(/overlap/);
    expect(sectionsHint(cut(2))).toMatch(/Parts overlap/);
  });
});

describe('the sections of a long document', () => {
  it('show all of them with no filter or pages up to 20', () => {
    show(headings(20));

    expect(screen.getAllByRole('listitem')).toHaveLength(20);
    expect(screen.queryByRole('searchbox')).toBeNull();
    expect(screen.queryByRole('navigation', { name: 'Pages' })).toBeNull();
  });

  it('come in pages past 20, and a filter searches every page', async () => {
    show(headings(45));

    expect(screen.getAllByRole('listitem')).toHaveLength(20);
    expect(screen.getByRole('status')).toHaveTextContent('1–20 of 45');

    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    expect(screen.getByRole('button', { name: 'Heading 21' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Previous' })).toBeEnabled();

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Filter sections' }), { target: { value: 'heading 4' } });

    const pages = screen.getByRole('navigation', { name: 'Pages' });

    expect(within(pages).getByRole('status')).toHaveTextContent('1–7 of 7');
    expect(screen.getByRole('button', { name: 'Heading 4' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Heading 21' })).toBeNull();

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Filter sections' }), { target: { value: 'nothing like this' } });
    expect(screen.getByText('No section matches this filter.')).toBeInTheDocument();
  });

  it('keep the part of a section when its page comes back', async () => {
    const sections = [...cut(3), ...headings(25).map((one) => ({ ...one, chunkId: (one.chunkId as number) + 10 }))];

    show(sections);
    await fireEvent.click(screen.getByRole('button', { name: 'Long' }));
    await fireEvent.click(await screen.findByRole('button', { name: 'Next part' }));
    expect(await screen.findByText('Text of part 2')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    expect(screen.queryByText('Text of part 2')).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Previous' }));
    expect(await screen.findByText('Text of part 2')).toBeInTheDocument();
  });
});
