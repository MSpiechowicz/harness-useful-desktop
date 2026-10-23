import { beforeEach, describe, expect, it } from 'vitest';

import type { AppPrefs } from '$lib/ipc/bindings/AppPrefs';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';
import type { NativeApprovalMode } from '$lib/ipc/bindings/NativeApprovalMode';
import { fixtures } from '$lib/ipc/mock/fixtures';

import { samplePrefs } from '../../../tests/helpers';
import { OnboardingController } from './onboarding-state.svelte';

/** The mock machine's harnesses with only the listed ones installed. */
function machine(installed: HarnessId[], claudeMode: NativeApprovalMode | null = null): HarnessSummary[] {
  return fixtures.harnesses.map((summary) => ({
    ...summary,
    detection: { ...summary.detection, installed: installed.includes(summary.info.id) },
    nativeApprovalMode: summary.info.id === 'claude' ? claudeMode : summary.nativeApprovalMode,
  }));
}

// The notices seen are remembered in the webview's storage: a test must not inherit the previous one's.
beforeEach(() => localStorage.clear());

function controller(installed: HarnessId[], prefs: Partial<AppPrefs> = {}, claudeMode: NativeApprovalMode | null = null) {
  const summaries = machine(installed, claudeMode);
  const harnesses = {
    summaries,
    installed: summaries.filter((summary) => summary.detection.installed),
    get: (id: HarnessId) => summaries.find((summary) => summary.info.id === id),
  };

  const preferences = {
    value: samplePrefs({ defaultHarness: null, approvalNoticeSeen: false, ...prefs }),
    update: async (patch: Partial<AppPrefs>) => {
      preferences.value = { ...preferences.value, ...patch };
    },
  };

  return new OnboardingController(preferences, harnesses, () => true);
}

describe('the daily driver', () => {
  it('prefers a harness that can chat when nothing was picked', () => {
    expect(controller(['opencode', 'pi']).driver).toBe('pi');
  });

  it('falls back to the harness without chat when it is the only one installed', () => {
    expect(controller(['opencode']).driver).toBe('opencode');
  });

  it('lets the user pick a harness without chat on purpose, and honours a saved one', () => {
    const picked = controller(['omp', 'opencode']);
    picked.choose('opencode');
    expect(picked.driver).toBe('opencode');

    expect(controller(['omp', 'opencode'], { defaultHarness: 'opencode' }).driver).toBe('opencode');
  });

  it('is nothing when no harness is installed', () => {
    expect(controller([]).driver).toBeNull();
  });
});

describe('the approval notice', () => {
  it('belongs to the daily driver: OMP, OpenCode, or none for the others', () => {
    expect(controller(['omp', 'opencode']).approvalNotice).toBe('omp');
    expect(controller(['opencode']).approvalNotice).toBe('opencode');
    expect(controller(['pi', 'opencode']).approvalNotice).toBeNull();
  });

  it('adds its step to the flow only when there is a notice, and only once', () => {
    expect(controller(['opencode']).steps).toContain('approval');
    expect(controller(['pi']).steps).not.toContain('approval');
    expect(controller(['omp'], { approvalNoticeSeen: true }).steps).not.toContain('approval');
  });

  it('is remembered per harness: seeing the OpenCode notice keeps the OMP one for its own driver', async () => {
    const opencode = controller(['opencode']);
    opencode.restart('approval');
    await opencode.finish();

    expect(opencode.approvalNotice).toBeNull();
    // A later launch with OMP as the driver still gets its notice.
    expect(controller(['omp']).approvalNotice).toBe('omp');
  });

  it('is remembered per harness: seeing the OMP notice keeps the OpenCode one', async () => {
    const omp = controller(['omp']);
    omp.restart('approval');
    await omp.finish();

    expect(controller(['opencode']).approvalNotice).toBe('opencode');
    expect(controller(['omp']).approvalNotice).toBeNull();
  });

  it('reads the saved flag from before there were two notices as the OMP one', () => {
    expect(controller(['omp'], { approvalNoticeSeen: true }).approvalNotice).toBeNull();
    expect(controller(['opencode'], { approvalNoticeSeen: true }).approvalNotice).toBe('opencode');
  });

  it('is not marked as seen when the run never reached it', async () => {
    const opencode = controller(['opencode']);
    opencode.restart('family');
    await opencode.finish();

    expect(opencode.approvalNotice).toBe('opencode');
  });
});

describe('the approval notice of Claude Code', () => {
  const BYPASS = { name: 'bypassPermissions', unattended: true };
  const AUTO = { name: 'auto', unattended: true };
  const ACCEPT_EDITS = { name: 'acceptEdits', unattended: false };

  it('is shown when Claude Code is the daily driver and its settings run tools without asking', () => {
    expect(controller(['claude'], {}, BYPASS).approvalNotice).toBe('claude');
    expect(controller(['claude'], {}, AUTO).approvalNotice).toBe('claude');
    expect(controller(['claude'], {}, BYPASS).steps).toContain('approval');
  });

  it('is not shown for a mode that asks, or for settings that choose none', () => {
    expect(controller(['claude'], {}, ACCEPT_EDITS).approvalNotice).toBeNull();
    expect(controller(['claude'], {}, null).approvalNotice).toBeNull();
    expect(controller(['claude'], {}, null).steps).not.toContain('approval');
  });

  it('is not shown when another harness is the daily driver', () => {
    expect(controller(['pi', 'claude'], {}, BYPASS).approvalNotice).toBeNull();
  });

  it('is shown once, and does not hide the notice of another harness', async () => {
    const claude = controller(['claude'], {}, BYPASS);
    claude.restart('approval');
    await claude.finish();

    expect(claude.approvalNotice).toBeNull();
    expect(controller(['claude'], {}, BYPASS).approvalNotice).toBeNull();
    expect(controller(['omp']).approvalNotice).toBe('omp');
  });
});

describe('the approval notice of Codex', () => {
  const NEVER = { name: 'never / workspace-write', unattended: true };
  const FULL_ACCESS = { name: 'on-request / danger-full-access', unattended: true };
  const ON_REQUEST = { name: 'on-request / workspace-write', unattended: false };

  /** A controller whose daily driver is Codex, in `mode`. */
  function codexDriver(mode: NativeApprovalMode | null) {
    const summaries = machine(['codex']).map((summary) => (summary.info.id === 'codex' ? { ...summary, nativeApprovalMode: mode } : summary));
    const harnesses = {
      summaries,
      installed: summaries.filter((summary) => summary.detection.installed),
      get: (id: HarnessId) => summaries.find((summary) => summary.info.id === id),
    };
    const preferences = {
      value: samplePrefs({ defaultHarness: null, approvalNoticeSeen: false }),
      update: async (patch: Partial<AppPrefs>) => {
        preferences.value = { ...preferences.value, ...patch };
      },
    };

    return new OnboardingController(preferences, harnesses, () => true);
  }

  it('is shown when its settings never ask or give commands full access', () => {
    expect(codexDriver(NEVER).approvalNotice).toBe('codex');
    expect(codexDriver(FULL_ACCESS).approvalNotice).toBe('codex');
    expect(codexDriver(NEVER).steps).toContain('approval');
  });

  it('is not shown for Codex’s default of asking on request', () => {
    expect(codexDriver(ON_REQUEST).approvalNotice).toBeNull();
    expect(codexDriver(null).approvalNotice).toBeNull();
  });
});
