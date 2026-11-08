import { describe, expect, it } from 'vitest';

import { OpenedVersion } from './opened-version.svelte';

interface Item {
  id: number;
  body: string;
}

/** A read the test answers by hand, so it can arrive after the editor moved on. */
function pendingRead() {
  let answer!: (value: Item | 'gone' | null) => void;
  const promise = new Promise<Item | 'gone' | null>((resolve) => (answer = resolve));

  return { promise, answer };
}

describe('reloading the item an editor is open on', () => {
  it('fills the form with the latest version', async () => {
    const version = new OpenedVersion<Item>();
    const filled: Item[] = [];

    version.open({ id: 1, body: 'old' });
    await version.reload(async () => ({ id: 1, body: 'new' }), (latest) => filled.push(latest));

    expect(filled).toEqual([{ id: 1, body: 'new' }]);
    expect(version.opened).toEqual({ id: 1, body: 'new' });
    expect(version.reloading).toBe(false);
  });

  it('leaves the form of the next item alone when the read of the one before answers late', async () => {
    const version = new OpenedVersion<Item>();
    const filled: Item[] = [];
    const late = pendingRead();

    version.open({ id: 1, body: 'first' });
    const reloading = version.reload(() => late.promise, (latest) => filled.push(latest));
    expect(version.reloading).toBe(true);

    // The editor is closed and opened on another item while the read is out.
    version.open({ id: 2, body: 'second' });
    late.answer({ id: 1, body: 'first, rewritten' });
    await reloading;

    expect(filled).toEqual([]);
    expect(version.opened).toEqual({ id: 2, body: 'second' });
    expect(version.stale).toBeNull();
    expect(version.reloading).toBe(false);
  });

  it('does not mark the next item gone when the read of the one before says it is', async () => {
    const version = new OpenedVersion<Item>();
    const late = pendingRead();

    version.open({ id: 1, body: 'first' });
    const reloading = version.reload(() => late.promise, () => undefined);

    version.open({ id: 2, body: 'second' });
    late.answer('gone');
    await reloading;

    expect(version.stale).toBeNull();
  });

  it('can reload the next item while the read of the one before is still out', async () => {
    const version = new OpenedVersion<Item>();
    const filled: Item[] = [];
    const late = pendingRead();

    version.open({ id: 1, body: 'first' });
    const first = version.reload(() => late.promise, (latest) => filled.push(latest));

    version.open({ id: 2, body: 'second' });
    await version.reload(async () => ({ id: 2, body: 'second, rewritten' }), (latest) => filled.push(latest));
    late.answer({ id: 1, body: 'first, rewritten' });
    await first;

    expect(filled).toEqual([{ id: 2, body: 'second, rewritten' }]);
    expect(version.opened).toEqual({ id: 2, body: 'second, rewritten' });
  });
});
