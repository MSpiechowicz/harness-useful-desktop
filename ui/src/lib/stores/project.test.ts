import { describe, expect, it } from 'vitest';

import { fakeTransport } from '../../../tests/helpers';
import { fixtures } from '../ipc/mock/fixtures';
import { createMockTransport } from '../ipc/mock/transport';
import { defaultScenario } from '../ipc/mock/scenarios';

import { PrefsStore } from './prefs.svelte';
import { ProjectStore } from './project.svelte';
import { ToastsStore } from './toasts.svelte';

/** Existing recent projects, most recent first, straight from the generated fixture. */
const RECENT = fixtures.recentProjects.filter((recent) => recent.exists);
const [FIRST, SECOND] = RECENT;

async function setup() {
  const transport = createMockTransport(defaultScenario(), 0);
  const toasts = new ToastsStore();
  const prefs = new PrefsStore(transport, toasts);
  await prefs.load();

  const store = new ProjectStore(transport, prefs, toasts);
  await store.load();

  return { store, toasts };
}

describe('ProjectStore', () => {
  it('starts without an active project and lists the recent ones', async () => {
    const { store } = await setup();

    expect(store.active).toBeNull();
    expect(store.recents).toEqual(fixtures.recentProjects);
  });

  it('opens a recent project and moves it to the front of the list', async () => {
    const { store } = await setup();

    await expect(store.open(SECOND.path)).resolves.toBe(true);

    expect(store.active?.path).toBe(SECOND.path);
    expect(store.recents[0].path).toBe(SECOND.path);
  });

  it('picks a new folder through the native dialog and remembers it', async () => {
    const { store } = await setup();

    const picked = await store.pick();

    expect(picked).not.toBeNull();
    expect(store.active?.path).toBe(picked?.path);
    expect(store.recents[0].path).toBe(picked?.path);
  });

  it('keeps the active project when the dialog is cancelled', async () => {
    const toasts = new ToastsStore();
    const transport = fakeTransport({ project_recent: () => [], project_pick: () => null });
    const store = new ProjectStore(transport, { value: null }, toasts);

    await expect(store.pick()).resolves.toBeNull();
    expect(store.active).toBeNull();
  });

  it('closes the project without forgetting it', async () => {
    const { store } = await setup();
    await store.open(FIRST.path);

    await store.close();

    expect(store.active).toBeNull();
    expect(store.recents.some((project) => project.path === FIRST.path)).toBe(true);
  });

  it('forgetting the active project removes it and clears the selection', async () => {
    const { store } = await setup();
    await store.open(FIRST.path);

    await store.forget(FIRST.path);

    expect(store.active).toBeNull();
    expect(store.recents.map((project) => project.path)).not.toContain(FIRST.path);
  });

  it('refuses a path that is not a recent project and tells the user', async () => {
    const { store, toasts } = await setup();

    await expect(store.open('/etc')).resolves.toBe(false);

    expect(store.active).toBeNull();
    expect(toasts.items[0]).toMatchObject({ tone: 'danger' });
  });

  it('restores the active project saved in the preferences', async () => {
    const transport = createMockTransport(defaultScenario(), 0);
    await transport.invoke('project_open', { path: FIRST.path });
    const toasts = new ToastsStore();
    const prefs = new PrefsStore(transport, toasts);
    await prefs.load();

    const store = new ProjectStore(transport, prefs, toasts);
    await store.load();

    expect(store.active?.path).toBe(FIRST.path);
  });
});
