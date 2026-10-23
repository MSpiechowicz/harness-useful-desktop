import { render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { harness } from '$lib/stores/harness.svelte';

import ApprovalStep from './ApprovalStep.svelte';

beforeEach(() => {
  harness.summaries = fixtures.harnesses.map((summary) =>
    summary.info.id === 'codex' ? { ...summary, nativeApprovalMode: { name: 'never / danger-full-access', unattended: true } } : summary,
  );
});

afterEach(() => {
  harness.summaries = fixtures.harnesses;
});

describe('the Codex approval notice', () => {
  it('names the policy and sandbox in the settings and promises the app never changes them', () => {
    render(ApprovalStep, { notice: 'codex' });

    expect(screen.getByRole('heading', { name: 'Codex can run commands without asking' })).toBeInTheDocument();
    expect(screen.getByText(/“never \/ danger-full-access”: commands run without asking/)).toBeInTheDocument();
    expect(screen.getByText('approval_policy')).toBeInTheDocument();
    expect(screen.getByText(/never changes Codex’s settings/)).toBeInTheDocument();
  });

  it('says Codex still asks when only the sandbox gives full access', () => {
    harness.summaries = fixtures.harnesses.map((summary) =>
      summary.info.id === 'codex' ? { ...summary, nativeApprovalMode: { name: 'on-request / danger-full-access', unattended: true } } : summary,
    );
    render(ApprovalStep, { notice: 'codex' });

    expect(screen.getByText(/“on-request \/ danger-full-access”: commands run with full access to your machine, without a sandbox\. Codex asks only when/)).toBeInTheDocument();
    expect(screen.queryByText(/commands run without asking/)).toBeNull();
  });

  it('lists the modes from asking to full access, with Codex’s default marked', () => {
    render(ApprovalStep, { notice: 'codex' });

    const modes = within(screen.getByRole('list', { name: 'Codex approval policies and sandboxes' })).getAllByRole('listitem');

    expect(modes.map((mode) => mode.textContent)).toEqual([
      expect.stringContaining('approval_policy = "untrusted"'),
      expect.stringContaining('approval_policy = "on-request"'),
      expect.stringContaining('sandbox_mode = "danger-full-access"'),
    ]);
    expect(within(modes[1]).getByText('Codex default')).toBeInTheDocument();
  });
});
