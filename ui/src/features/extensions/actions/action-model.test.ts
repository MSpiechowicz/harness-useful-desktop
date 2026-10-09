import { describe, expect, it } from 'vitest';

import type { CliResult } from '$lib/ipc/bindings/CliResult';
import type { ExtensionsView } from '$lib/ipc/bindings/ExtensionsView';
import { fixtures } from '$lib/ipc/mock/fixtures';

import { actionRequest, failureLine, hasNoPreview, outcomeIn, targetProblem } from './action-model';

function result(fields: Partial<CliResult>): CliResult {
  return { ok: true, summary: null, exitCode: 0, stdout: '', stderr: '', truncated: false, timedOut: false, argv: ['omp'], command: ['omp'], ...fields };
}

describe('failureLine', () => {
  it('reports a failure the CLI printed even though it exited with 0', () => {
    const line = '✘ Failed to disable x: Error: not found';

    expect(failureLine(result({ ok: true, stderr: `${line}\n` }))).toBe(line);
    expect(failureLine(result({ ok: true, stdout: 'Failed to upgrade: offline\n' }))).toBe('Failed to upgrade: offline');
  });

  it('trusts the backend verdict and a clean output', () => {
    expect(failureLine(result({ ok: false, summary: 'The command exited with code 2.', exitCode: 2 }))).toBe('The command exited with code 2.');
    expect(failureLine(result({ stdout: '✔ Installed x\n' }))).toBeNull();
  });
});

describe('no preview', () => {
  it('is a run that never started, not a killed one', () => {
    expect(hasNoPreview(result({ exitCode: null }))).toBe(true);
    expect(hasNoPreview(result({ exitCode: null, timedOut: true }))).toBe(false);
    expect(hasNoPreview(result({ exitCode: 0 }))).toBe(false);
  });
});

describe('actionRequest', () => {
  it('carries the project folder only for project scope', () => {
    const action = { type: 'pluginInstall', target: 'a@b', scope: 'project' } as const;

    expect(actionRequest('omp', action, 'project', '/p').project).toBe('/p');
    expect(actionRequest('omp', action, 'user', '/p').project).toBeNull();
  });

  it('marks removals as destructive and names the target', () => {
    const request = actionRequest('omp', { type: 'marketplaceRemove', name: 'demo' }, 'user', null);

    expect(request).toMatchObject({ destructive: true, title: 'Remove marketplace demo', verb: 'Remove marketplace' });
    expect(actionRequest('pi', { type: 'packageUpdate', source: null }, 'user', null).title).toBe('Update all packages');
  });
});

describe('outcomeIn', () => {
  const view = fixtures.extensions.omp as ExtensionsView;

  it('compares the refreshed list with what the action meant to do', () => {
    expect(outcomeIn(view, { type: 'pluginDisable', name: 'npm-helper' })).toBe('confirmed');
    expect(outcomeIn(view, { type: 'pluginEnable', name: 'npm-helper' })).toBe('missing');
    expect(outcomeIn(view, { type: 'pluginUninstall', name: 'gone-plugin' })).toBe('confirmed');
    expect(outcomeIn(view, { type: 'pluginInstall', target: 'demo-plugin@demo', scope: 'user' })).toBe('confirmed');
    expect(outcomeIn(view, { type: 'marketplaceRemove', name: 'demo' })).toBe('missing');
  });

  it('does not judge updates, which change no visible state', () => {
    expect(outcomeIn(view, { type: 'pluginUpgrade', name: null })).toBe('unchecked');
  });
});

describe('targetProblem', () => {
  it('accepts what the backend accepts', () => {
    for (const ok of ['demo@market', 'npm:@scope/pkg@1.2.0', 'git:github.com/a/b', 'owner/repo']) {
      expect(targetProblem(ok)).toBeNull();
    }
  });

  it('refuses what the backend would refuse', () => {
    expect(targetProblem('  ')).toBe('Enter a value.');
    expect(targetProblem('-x')).toMatch(/dash/);
    expect(targetProblem('a b')).toMatch(/only letters/);
    expect(targetProblem('https://user:pw@host/repo')).toMatch(/credentials/);
    expect(targetProblem('x'.repeat(513))).toMatch(/too long/);
  });
});
