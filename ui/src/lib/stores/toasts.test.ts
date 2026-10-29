import { render } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import ToastHost from '$lib/components/ToastHost.svelte';

import { toasts, ToastsStore } from './toasts.svelte';

describe('ToastsStore.update', () => {
  it('changes the given keys of a shown toast and keeps the rest', () => {
    const store = new ToastsStore();
    const id = store.push({ title: 'Building', description: '1 of 9', duration: 0, project: '/p' });

    store.update(id, { description: '5 of 9' });
    store.update(id, { title: 'Ready', tone: 'success', duration: 4000 });

    expect(store.items).toEqual([{ id, title: 'Ready', description: '5 of 9', tone: 'success', duration: 4000, action: undefined, project: '/p' }]);
  });

  it('clears the description when it is passed as undefined', () => {
    const store = new ToastsStore();
    const id = store.push({ title: 'Ready', description: 'old' });

    store.update(id, { description: undefined });

    expect(store.items[0].description).toBeUndefined();
  });

  it('does nothing for a toast that is gone', () => {
    const store = new ToastsStore();
    const id = store.push({ title: 'Gone' });

    store.dismiss(id);
    store.update(id, { title: 'Back' });

    expect(store.items).toEqual([]);
  });
});

describe('a toast whose duration is updated', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    toasts.items = [];
    vi.useRealTimers();
  });

  it('stays while it has no duration and leaves after the new one', async () => {
    const view = render(ToastHost);
    const id = toasts.push({ title: 'Building', duration: 0 });

    await vi.advanceTimersByTimeAsync(60_000);
    expect(toasts.items.map((item) => item.id)).toEqual([id]);

    toasts.update(id, { title: 'Ready', duration: 4000 });
    await vi.advanceTimersByTimeAsync(3900);
    expect(toasts.items).toHaveLength(1);

    await vi.advanceTimersByTimeAsync(200);
    expect(toasts.items).toEqual([]);

    view.unmount();
  });
});
