import { fireEvent, render } from '@testing-library/svelte';
import { createRawSnippet, tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import VirtualList from './VirtualList.svelte';

const row = createRawSnippet(() => ({ render: () => '<div><button type="button">row</button></div>' }));
const VIEWPORT = 400;

const rows = (count: number) => Array.from({ length: count }, (_, index) => index);

// The list writes its follow position in an animation frame; the test moves that clock itself.
beforeEach(() => vi.useFakeTimers({ toFake: ['requestAnimationFrame', 'cancelAnimationFrame'] }));
afterEach(() => vi.useRealTimers());

async function settle(): Promise<void> {
  await tick();
  vi.advanceTimersByTime(20);
}

/** The list element with the geometry of a scrolled box, which happy-dom does not lay out. */
function mount(count: number) {
  const view = render(VirtualList, { props: { items: rows(count), label: 'Rows', stick: true, estimate: 50, row } });
  const list = view.getByRole('list') as HTMLElement;
  const box = { height: count * 50, top: 0 };

  Object.defineProperty(list, 'scrollHeight', { get: () => box.height, configurable: true });
  Object.defineProperty(list, 'clientHeight', { get: () => VIEWPORT, configurable: true });
  Object.defineProperty(list, 'scrollTop', {
    get: () => box.top,
    set: (value: number) => {
      box.top = Math.min(value, box.height - VIEWPORT);
      // A browser reports a changed position in a scroll event of its own.
      queueMicrotask(() => list.dispatchEvent(new Event('scroll')));
    },
    configurable: true,
  });

  return {
    list,
    box,
    /** The browser moved the list: it reports where it is now. */
    async scrollTo(top: number) {
      box.top = top;
      await fireEvent.scroll(list);
    },
    /** Content grew: the new height shows once the list re-renders. */
    async grow(next: number) {
      box.height = next * 50;
      await view.rerender({ items: rows(next) });
      await settle();
    },
  };
}

describe('VirtualList follow mode', () => {
  it('keeps the end in view as rows are added', async () => {
    const view = mount(20);
    await view.grow(30);

    expect(view.box.top).toBe(30 * 50 - VIEWPORT);
  });

  it('stops following after even a one-pixel wheel turn up from the end', async () => {
    const view = mount(20);
    await view.grow(21);
    const end = view.box.top;

    await fireEvent.wheel(view.list, { deltaY: -1 });
    await view.scrollTo(end - 1);
    await view.grow(25);

    expect(view.box.top).toBe(end - 1);
  });

  it('follows again once the person scrolls back down to the end', async () => {
    const view = mount(20);
    await view.grow(21);
    const end = view.box.top;

    await fireEvent.wheel(view.list, { deltaY: -40 });
    await view.scrollTo(end - 40);
    await view.scrollTo(end);
    await view.grow(25);

    expect(view.box.top).toBe(25 * 50 - VIEWPORT);
  });

  it('keeps following when the list moves up without the person asking, as when content shrinks', async () => {
    const view = mount(20);
    await view.grow(21);

    await view.scrollTo(view.box.top - 30);
    await view.grow(25);

    expect(view.box.top).toBe(25 * 50 - VIEWPORT);
  });

  it('keeps following when a wheel turn scrolls something inside a row and the list does not move', async () => {
    const view = mount(20);
    await view.grow(21);

    await fireEvent.wheel(view.list.querySelector('button')!, { deltaY: -40 });
    await view.grow(25);

    expect(view.box.top).toBe(25 * 50 - VIEWPORT);
  });

  it('does not count a zoom gesture (ctrl + wheel) as a scroll up', async () => {
    const view = mount(20);
    await view.grow(21);

    const zoom = new WheelEvent('wheel', { deltaY: -40, bubbles: true });
    // happy-dom drops `ctrlKey` from the init dictionary.
    Object.defineProperty(zoom, 'ctrlKey', { value: true });
    await fireEvent(view.list, zoom);
    await view.scrollTo(view.box.top - 30);
    await view.grow(25);

    expect(view.box.top).toBe(25 * 50 - VIEWPORT);
  });

  it('stops following on Page Up even when focus is on the page, not in the list', async () => {
    const view = mount(20);
    await view.grow(21);
    const end = view.box.top;

    await fireEvent.keyDown(document.body, { key: 'PageUp' });
    await view.scrollTo(end - VIEWPORT);
    await view.grow(25);

    expect(view.box.top).toBe(end - VIEWPORT);
  });

  it('does not take arrow keys typed into a field for scrolling', async () => {
    const view = mount(20);
    await view.grow(21);
    const field = document.createElement('input');
    document.body.append(field);

    await fireEvent.keyDown(field, { key: 'ArrowUp' });
    await view.scrollTo(view.box.top - 30);
    await view.grow(25);

    expect(view.box.top).toBe(25 * 50 - VIEWPORT);
    field.remove();
  });

  it('stops following while the scrollbar is held and moves up', async () => {
    const view = mount(20);
    await view.grow(21);
    const end = view.box.top;

    await fireEvent.pointerDown(view.list);
    await view.scrollTo(end - 200);
    await fireEvent.pointerUp(window);
    await view.grow(25);

    expect(view.box.top).toBe(end - 200);
  });
});
