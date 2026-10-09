// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { openExternal } = vi.hoisted(() => ({ openExternal: vi.fn() }));
vi.mock('$lib/ipc/actions', () => ({ openExternal }));

import Markdown from './Markdown.svelte';

beforeEach(() => {
  openExternal.mockReset();
});

describe('Markdown', () => {
  it('shows raw HTML as text and creates no elements from it', () => {
    const { container } = render(Markdown, { source: 'Hello <img src=x onerror="alert(1)"> <script>alert(2)</script>' });

    expect(container.querySelector('img, script')).toBeNull();
    expect(container.textContent).toContain('<script>alert(2)</script>');
  });

  it('opens web links through the backend and never navigates', async () => {
    render(Markdown, { source: 'Read [the book](https://doc.rust-lang.org/book/).' });

    const link = screen.getByRole('button', { name: 'the book' });
    const proceeded = fireEvent.click(link);

    await expect(proceeded).resolves.toBe(false);
    expect(openExternal).toHaveBeenCalledWith('https://doc.rust-lang.org/book/');
  });

  it('does not make javascript: links clickable', async () => {
    const { container } = render(Markdown, { source: 'Try [this](javascript:alert(1)) now' });

    expect(screen.queryByRole('button')).toBeNull();
    expect(container.querySelector('a')).toBeNull();

    await fireEvent.click(screen.getByText(/javascript:alert\(1\)/));

    expect(openExternal).not.toHaveBeenCalled();
  });

  it('shows code at once and highlights it afterwards', async () => {
    const { container } = render(Markdown, { source: '```ts\nconst answer: number = 42;\n```' });

    expect(container.querySelector('pre code')?.textContent).toBe('const answer: number = 42;');
    expect(container.querySelectorAll('pre code span')).toHaveLength(0);

    await waitFor(() => expect(container.querySelectorAll('pre code span').length).toBeGreaterThan(2), { timeout: 8000 });

    expect(container.querySelector('pre code')?.textContent).toBe('const answer: number = 42;');
    expect(Array.from(container.querySelectorAll<HTMLElement>('pre code span')).some((span) => span.style.color !== '')).toBe(true);
  });

  it('leaves a block of an unknown language plain', async () => {
    const { container } = render(Markdown, { source: '```klingon\nqapla\n```' });

    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(container.querySelectorAll('pre code span')).toHaveLength(0);
    expect(container.querySelector('pre code')?.textContent).toBe('qapla');
  });
});
