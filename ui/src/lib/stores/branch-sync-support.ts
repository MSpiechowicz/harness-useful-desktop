import { vi } from 'vitest';

import type { BranchPage } from '$lib/ipc/bindings/BranchPage';
import type { BranchSync } from '$lib/ipc/bindings/BranchSync';
import { fixtures } from '$lib/ipc/mock/fixtures';

import { fakeTransport } from '../../../tests/helpers';
import { BranchSyncStore } from './branch-sync.svelte';
import { ToastsStore } from './toasts.svelte';

export const PROJECT = '/home/demo/project';

export function sync(overrides: Partial<BranchSync> = {}): BranchSync {
  return { ...structuredClone(fixtures.git.sync), ...overrides };
}

export type Handlers = Parameters<typeof fakeTransport>[0];

export function build(handlers: Handlers, { allowed = true }: { allowed?: boolean } = {}) {
  const notifications = new ToastsStore();
  const repository = { refresh: vi.fn(async () => {}) };
  const store = new BranchSyncStore(fakeTransport(handlers), notifications, repository, () => allowed);

  return { store, notifications, repository };
}

/** A reply the test settles when it wants to, to put answers in a chosen order. */
export function deferred<T>() {
  let settle: (value: T) => void = () => {};
  const promise = new Promise<T>((resolve) => (settle = resolve));

  return { promise, settle };
}

/** `count` local branches named `feat/<first + i>`, as one slice of a search. */
export function rows(first: number, count: number): BranchPage['branches'] {
  return Array.from({ length: count }, (_, index) => ({
    name: `feat/${first + index}`,
    isCurrent: false,
    remoteOnly: false,
    lastCommitDate: null,
  }));
}
