import { render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { harness } from '$lib/stores/harness.svelte';

import ApprovalStep from './ApprovalStep.svelte';

beforeEach(() => {
  harness.summaries = fixtures.harnesses.map((summary) =>
    summary.info.id === 'claude' ? { ...summary, nativeApprovalMode: { name: 'bypassPermissions', unattended: true } } : summary,
  );
});

afterEach(() => {
  harness.summaries = fixtures.harnesses;
});

describe('the Claude Code approval notice', () => {
  it('names the mode in the settings and promises the app never changes the permissions', () => {
    render(ApprovalStep, { notice: 'claude' });

    expect(screen.getByRole('heading', { name: 'Claude Code can run tools without asking' })).toBeInTheDocument();
    expect(screen.getByText(/“Bypass permissions” mode: tools run without asking/)).toBeInTheDocument();
    expect(screen.getByText('permissions.defaultMode')).toBeInTheDocument();
    expect(screen.getByText(/never changes Claude Code’s permissions/)).toBeInTheDocument();
  });

  it('lists the modes from asking to nothing waiting', () => {
    render(ApprovalStep, { notice: 'claude' });

    const modes = within(screen.getByRole('list', { name: 'Claude Code permission modes' })).getAllByRole('listitem');

    expect(modes.map((mode) => mode.textContent)).toEqual([
      expect.stringContaining('Default'),
      expect.stringContaining('Accept edits'),
      expect.stringContaining('Bypass permissions'),
    ]);
  });
});
