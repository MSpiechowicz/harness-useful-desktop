import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { Hook } from '$lib/ipc/bindings/Hook';
import type { HookView } from '$lib/ipc/bindings/HookView';
import type { ShareBlock } from '$lib/ipc/bindings/ShareBlock';
import { LOCAL_ORIGIN } from '$lib/ipc/mock/hooks-team-state';

import HookActions from './HookActions.svelte';

const HOOK: Hook = {
  id: 'c'.repeat(32),
  name: 'Keep commits small',
  description: null,
  scope: 'global',
  project: null,
  event: 'turnEnd',
  matcher: null,
  action: { type: 'followUp', text: 'Suggest splitting the commit.' },
  enabled: true,
  requires: [],
  seed: null,
  createdAt: '2026-10-17T10:00:00Z',
  createdBy: 'desktop',
  updatedAt: '2026-10-17T10:00:00Z',
  updatedBy: 'desktop',
};

function local(shareBlock: ShareBlock | null): HookView {
  return { id: HOOK.id, hook: HOOK, name: HOOK.name, status: 'active', sha256: 'a'.repeat(64), problem: null, approval: 'approved', approved: null, ...LOCAL_ORIGIN, shareBlock };
}

function team(patch: Partial<HookView> = {}): HookView {
  return { ...local(null), origin: 'team', teamAuthor: 'Anna', teamUpdatedAt: '2026-10-17T10:00:00Z', ...patch };
}

function show(view: HookView, props: { sharing?: boolean; canUpdate?: boolean } = {}) {
  const handlers = { onedit: vi.fn(), ondelete: vi.fn(), onshare: vi.fn(), onupdate: vi.fn(), onremove: vi.fn(), oncopy: vi.fn() };

  render(HookActions, { view, lockReason: null, sharing: props.sharing ?? true, canUpdate: props.canUpdate ?? true, busy: false, ...handlers });

  return handlers;
}

describe('Share with team', () => {
  it('shares a hook that can be shared', async () => {
    const { onshare } = show(local(null));

    await fireEvent.click(screen.getByRole('button', { name: 'Share with team' }));

    expect(onshare).toHaveBeenCalledOnce();
  });

  it('is not offered while hooks are not shared with a team', () => {
    show(local(null), { sharing: false });

    expect(screen.queryByRole('button', { name: 'Share with team' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Edit' })).toBeEnabled();
  });

  it.each<[ShareBlock, string]>([
    ['seed', 'Harness family hooks are never shared. Every computer adds its own.'],
    ['pending', 'Approve this hook first. Only hooks you approved can be shared.'],
    ['invalid', 'This file does not follow the hook format, so it cannot be shared.'],
    ['noOrigin', 'This repository has no origin remote, so its hooks cannot be shared.'],
    ['onTeam', 'Already on remote. Use Update remote copy on the remote hook.'],
  ])('is off for %s, with the reason as its tooltip', async (block, reason) => {
    const { onshare } = show(local(block));
    const button = screen.getByRole('button', { name: 'Share with team' });

    expect(button).toBeDisabled();

    await fireEvent.pointerEnter(button.parentElement as HTMLElement);

    expect(await screen.findByRole('tooltip')).toHaveTextContent(reason);
    await fireEvent.click(button);
    expect(onshare).not.toHaveBeenCalled();
  });
});

describe('The actions of a remote hook', () => {
  it('copy, update and remove it, and never edit or delete it', async () => {
    const { oncopy, onupdate, onremove } = show(team({ teamChanged: true }));

    expect(screen.queryByRole('button', { name: 'Edit' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Delete' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Share with team' })).not.toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Copy to my hooks' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Update remote copy' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Remove from remote' }));

    expect([oncopy, onupdate, onremove].map((handler) => handler.mock.calls.length)).toEqual([1, 1, 1]);
  });

  it.each<[HookView['status'], string]>([
    ['pending', 'Approve it first. A remote hook you have not approved cannot be copied.'],
    ['duplicate', 'You already have a hook with this id.'],
    ['invalid', 'This remote hook does not follow the hook format, so it cannot be copied.'],
  ])('cannot be copied while %s, with the reason as a tooltip', async (status, reason) => {
    const { oncopy } = show(team({ status }));
    const copy = screen.getByRole('button', { name: 'Copy to my hooks' });

    expect(copy).toBeDisabled();

    await fireEvent.pointerEnter(copy.parentElement as HTMLElement);

    expect(await screen.findByRole('tooltip')).toHaveTextContent(reason);
    await fireEvent.click(copy);
    expect(oncopy).not.toHaveBeenCalled();
  });

  it('offer the update only when the original changed since it was shared, and is still there', () => {
    show(team({ teamChanged: false }));

    expect(screen.queryByRole('button', { name: 'Update remote copy' })).not.toBeInTheDocument();
  });

  it('leave the update out when the original is gone', () => {
    show(team({ teamChanged: true }), { canUpdate: false });

    expect(screen.queryByRole('button', { name: 'Update remote copy' })).not.toBeInTheDocument();
  });
});
