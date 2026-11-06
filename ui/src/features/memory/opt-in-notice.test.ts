import { afterEach, describe, expect, it, vi } from 'vitest';

import { IpcFailure } from '$lib/ipc/transport';
import { nav } from '$lib/stores/nav.svelte';
import { ToastsStore } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { fakeTransport, samplePrefs } from '../../../tests/helpers';
import { type OptInFacts, MemoryOptInNotice } from './opt-in-notice';

/** An existing user whose Harness Memory applies the opt-in guards: the one who has something to learn. */
const EXISTING: OptInFacts = { done: false, enabled: true, installed: true, supported: true };

function setup(facts: OptInFacts, mark: () => Promise<unknown> = async () => samplePrefs({ memoryOptInNoticeDone: true })) {
  const toasts = new ToastsStore();
  const marked = vi.fn(mark);
  const saved = vi.fn();
  let current = facts;
  const notice = new MemoryOptInNotice({
    transport: fakeTransport({ memory_opt_in_notice_done: async () => marked() }),
    notifications: toasts,
    facts: () => current,
    saved,
    openSettings: () => {
      viewState.settingsTarget = 'memory';
      nav.go('settings');
    },
  });

  return { notice, toasts, marked, saved, change: (next: OptInFacts) => (current = next) };
}

afterEach(() => {
  viewState.settingsTarget = null;
  vi.restoreAllMocks();
});

describe('the notice that saving notes and scripts is opt-in', () => {
  it('tells an existing user once, stays until closed, and marks it done as it is shown', async () => {
    const { notice, toasts, marked, saved } = setup(EXISTING);

    await notice.run();
    await notice.run();

    expect(marked).toHaveBeenCalledTimes(1);
    expect(saved).toHaveBeenCalledWith(expect.objectContaining({ memoryOptInNoticeDone: true }));
    expect(toasts.items).toHaveLength(1);
    expect(toasts.items[0]).toMatchObject({
      title: 'Notes and scripts are off until you turn them on',
      description: 'Agents still read what is saved. Choose where they may save in Settings.',
      tone: 'neutral',
      duration: 0,
      action: { label: 'Open Settings' },
    });
  });

  it('opens Settings at the Memory section from its action', async () => {
    const { notice, toasts } = setup(EXISTING);
    await notice.run();

    toasts.items[0].action?.run();

    expect(viewState.settingsTarget).toBe('memory');
    expect(nav.current).toBe('settings');
  });

  it('never shows for a user who has seen it, nor for a fresh install', async () => {
    const { notice, toasts, marked } = setup({ ...EXISTING, done: true });

    await notice.run();

    expect(toasts.items).toEqual([]);
    expect(marked).not.toHaveBeenCalled();
  });

  it('waits until what decides it is known', async () => {
    const { notice, toasts, marked, change } = setup({ done: null, enabled: null, installed: null, supported: null });

    await notice.run();
    change({ done: false, enabled: true, installed: null, supported: null });
    await notice.run();
    change({ done: false, enabled: true, installed: true, supported: null });
    await notice.run();

    expect(marked).not.toHaveBeenCalled();
    expect(toasts.items).toEqual([]);

    change(EXISTING);
    await notice.run();

    expect(toasts.items).toHaveLength(1);
  });

  it('marks it done without a word for a user who has no Harness Memory', async () => {
    const { notice, toasts, marked } = setup({ done: false, enabled: true, installed: false, supported: null });

    await notice.run();

    expect(marked).toHaveBeenCalledTimes(1);
    expect(toasts.items).toEqual([]);
  });

  it('holds it back while Project memory is off, and tells once it is on', async () => {
    const { notice, toasts, marked, change } = setup({ ...EXISTING, enabled: false });

    await notice.run();
    expect(marked).toHaveBeenCalledTimes(0);
    expect(toasts.items).toEqual([]);

    change(EXISTING);
    await notice.run();
    expect(marked).toHaveBeenCalledTimes(1);
    expect(toasts.items).toHaveLength(1);
  });

  it('holds it back while the installed Harness Memory cannot apply the opt-in, and tells once it can', async () => {
    const { notice, toasts, marked, change } = setup({ done: false, enabled: true, installed: true, supported: false });

    await notice.run();

    expect(marked).not.toHaveBeenCalled();
    expect(toasts.items).toEqual([]);

    change(EXISTING);
    await notice.run();

    expect(toasts.items).toHaveLength(1);
  });

  it('logs and shows nothing when it cannot be marked done, so it is not shown without being remembered', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const { notice, toasts, saved } = setup(EXISTING, async () => {
      throw new IpcFailure({ code: 'io', message: 'the preferences are read-only' });
    });

    await notice.run();

    expect(warn).toHaveBeenCalledTimes(1);
    expect(toasts.items).toEqual([]);
    expect(saved).not.toHaveBeenCalled();
  });
});
