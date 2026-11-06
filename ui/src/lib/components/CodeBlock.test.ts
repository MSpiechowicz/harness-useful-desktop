import { render, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { highlight } from '$lib/markdown/highlight';

import CodeBlock from './CodeBlock.svelte';

const CODE = 'const answer: number = 42;';

/** The observers of the blocks on the page: `scrollIn` tells one that its block came near the screen. */
const observers: { scrollIn: () => void; disconnected: boolean }[] = [];

beforeEach(() => {
  observers.length = 0;
  vi.stubGlobal(
    'IntersectionObserver',
    class {
      private readonly entry = { scrollIn: () => {}, disconnected: false };

      constructor(callback: IntersectionObserverCallback) {
        this.entry.scrollIn = () => callback([{ isIntersecting: true } as IntersectionObserverEntry], this as unknown as IntersectionObserver);
        observers.push(this.entry);
      }

      observe(): void {}

      disconnect(): void {
        this.entry.disconnected = true;
      }
    },
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
});

const spans = (container: Element): number => container.querySelectorAll('pre code span').length;

describe('a code block off the screen', () => {
  it('stays plain text and leaves the highlighter alone until it comes near', async () => {
    const { container } = render(CodeBlock, { code: CODE, lang: 'ts' });

    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(spans(container)).toBe(0);
    expect(container.querySelector('pre code')?.textContent).toBe(CODE);

    observers[0].scrollIn();

    await waitFor(() => expect(spans(container)).toBeGreaterThan(2), { timeout: 8000 });
    expect(container.querySelector('pre code')?.textContent).toBe(CODE);
    expect(observers[0].disconnected).toBe(true);
  });

  it('highlights only the block that came near', async () => {
    const first = render(CodeBlock, { code: CODE, lang: 'ts' });
    const second = render(CodeBlock, { code: CODE, lang: 'ts' });

    observers[1].scrollIn();

    await waitFor(() => expect(spans(second.container)).toBeGreaterThan(2), { timeout: 8000 });
    expect(spans(first.container)).toBe(0);
  });

  it('stops watching when it goes away', () => {
    const { unmount } = render(CodeBlock, { code: CODE, lang: 'ts' });

    unmount();

    expect(observers[0].disconnected).toBe(true);
  });
});

describe('where nothing can watch the screen', () => {
  it('highlights at once', async () => {
    vi.stubGlobal('IntersectionObserver', undefined);

    const { container } = render(CodeBlock, { code: CODE, lang: 'ts' });

    await waitFor(() => expect(spans(container)).toBeGreaterThan(2), { timeout: 8000 });
    expect(observers).toHaveLength(0);
  });
});

describe('highlighting a very long line', () => {
  it('keeps the text and does not tokenize it, so it cannot block the UI', async () => {
    const long = `const list = [${'"item", '.repeat(5_000)}];`;

    const lines = await highlight(`const short = 1;\n${long}`, 'javascript');

    expect(lines?.map((line) => line.map((token) => token.content).join(''))).toEqual(['const short = 1;', long]);
    expect(lines?.[0].length).toBeGreaterThan(2);
    expect(lines?.[1]).toHaveLength(1);
  });
});
