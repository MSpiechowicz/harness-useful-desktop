import { render, screen, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { Hook } from '$lib/ipc/bindings/Hook';
import type { HookView } from '$lib/ipc/bindings/HookView';
import { LOCAL_ORIGIN } from '$lib/ipc/mock/hooks-team-state';

import HookReview from './HookReview.svelte';

const APPROVED: Hook = {
  id: 'b'.repeat(32),
  name: 'Run lint after edits',
  description: 'Catches style problems.',
  scope: 'project',
  project: '/work/app',
  event: 'toolEnd',
  matcher: 'edit',
  action: { type: 'command', command: 'bun run lint', timeoutSecs: null },
  enabled: true,
  requires: [],
  seed: null,
  createdAt: '2026-10-17T10:00:00Z',
  createdBy: 'agent',
  updatedAt: '2026-10-17T10:00:00Z',
  updatedBy: 'desktop',
};

function review(waiting: Hook): HTMLElement {
  const view: HookView = { id: waiting.id, hook: waiting, name: waiting.name, status: 'pending', sha256: 'c'.repeat(64), problem: null, approval: 'changed', approved: APPROVED, ...LOCAL_ORIGIN, shareBlock: null };

  return render(HookReview, { view, projects: [], busy: false, onapprove: () => {}, onreject: () => {} }).container;
}

describe('Hook review', () => {
  it('shows no empty box of changes when no part the user reads differs, only the notice and its buttons', () => {
    // only who saved it and when differ: nothing to list, but the version still waits for approval
    const container = review({ ...APPROVED, updatedAt: '2026-10-30T10:00:00Z', updatedBy: 'agent' });

    // the dark box itself (bg-surface-1) is what looked empty
    expect(container.querySelector('[data-review-changes], .bg-surface-1')).toBeNull();
    expect(screen.getByRole('button', { name: 'Approve' })).toBeInTheDocument();
  });

  it('keeps the box of changes when a part differs', () => {
    const container = review({ ...APPROVED, name: 'Run lint and tests', updatedBy: 'agent' });

    expect(container.querySelector('[data-review-changes]')).not.toBeNull();
  });

  it('shows the folder an agent moved a hook to, in full, and the project it applies to now, next to the ones the user approved', () => {
    review({ ...APPROVED, project: '/work/other-app', updatedBy: 'agent' });

    const group = screen.getByRole('group', { name: 'Changes to Project' });

    expect(within(group).getByText('/work/app')).toBeInTheDocument();
    expect(within(group).getByText('/work/other-app')).toBeInTheDocument();

    const scope = screen.getByRole('group', { name: 'Changes to Applies to' });

    expect(within(scope).getByText('app')).toBeInTheDocument();
    expect(within(scope).getByText('other-app')).toBeInTheDocument();
  });

  it('spells out the characters an agent hid in a name or a description', () => {
    review({ ...APPROVED, name: 'Run lint‮ after edits', description: 'Catches​ style problems.', updatedBy: 'agent' });

    expect(within(screen.getByRole('group', { name: 'Changes to Name' })).getByText('Run lint⟨U+202E⟩ after edits')).toBeInTheDocument();
    expect(within(screen.getByRole('group', { name: 'Changes to Description' })).getByText('Catches⟨U+200B⟩ style problems.')).toBeInTheDocument();
  });
});
