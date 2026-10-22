import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';
import type { Routine } from '$lib/ipc/bindings/Routine';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { withOpenCodeChat } from '$lib/ipc/mock/opencode-chat';
import { harness } from '$lib/stores/harness.svelte';

import { blankForm, draftOf, formOf, hasHiddenCharacters, problemsOf, type RoutineForm, runsUnattended, targetOf, toggleDraft, whenOf, withHarness, withTarget } from './routine-form';

// The harness list says what each harness lets a routine ask for.
beforeEach(() => {
  harness.summaries = fixtures.harnesses.map(withOpenCodeChat);
});

afterEach(() => {
  harness.summaries = [];
});

const CONTEXT = { projectChosen: true, scopeFull: false };

const AGENT: Routine = {
  id: 'a'.repeat(32),
  name: 'Morning review',
  description: 'Goes through the night.',
  scope: 'project',
  project: '/work/app',
  when: { type: 'weekdays', at: '09:00' },
  action: { type: 'agent', harness: 'omp', model: { provider: 'anthropic', id: 'claude-sonnet-4-5' }, prompt: 'Review the open pull requests.', approvalMode: 'write', trustOnce: false, maxMinutes: 20 },
  enabled: true,
  createdAt: '2026-10-17T10:00:00Z',
  createdBy: 'desktop',
  updatedAt: '2026-10-17T10:00:00Z',
  updatedBy: 'desktop',
};

function complete(patch: Partial<RoutineForm> = {}): RoutineForm {
  return { ...blankForm(), name: 'Archive logs', prompt: 'Pack up the logs.', ...patch };
}

function fieldsWithProblems(form: RoutineForm, context = CONTEXT): string[] {
  return problemsOf(form, context).map((problem) => problem.field);
}

describe('the form of an existing routine', () => {
  it('holds every field as the routine has it, and writes the same routine back', () => {
    const form = formOf(AGENT);

    expect(form).toMatchObject({ name: 'Morning review', whenKind: 'weekdays', at: '09:00', kind: 'agent', harness: 'omp', provider: 'anthropic', modelId: 'claude-sonnet-4-5', approvalMode: 'write', maxMinutes: '20', scope: 'project', projectPath: '/work/app' });
    expect(draftOf(form, { id: AGENT.id, sha256: 'f'.repeat(64) })).toEqual({
      id: AGENT.id,
      expectedSha256: 'f'.repeat(64),
      name: 'Morning review',
      description: 'Goes through the night.',
      scope: 'project',
      project: '/work/app',
      when: { type: 'weekdays', at: '09:00' },
      action: AGENT.action,
      enabled: true,
    });
  });

  it.each([
    [{ type: 'everyMinutes', every: 45 }, { whenKind: 'everyMinutes', every: '45' }],
    [{ type: 'everyHours', every: 2 }, { whenKind: 'everyHours', every: '2' }],
    [{ type: 'cron', expression: '0 8 * * 1' }, { whenKind: 'cron', cron: '0 8 * * 1' }],
    [{ type: 'appStart' }, { whenKind: 'appStart' }],
  ] as const)('reads the schedule %j', (when, fields) => {
    expect(formOf({ ...AGENT, when })).toMatchObject(fields);
    expect(whenOf(formOf({ ...AGENT, when }))).toEqual(when);
  });

  it('reads a command and a script', () => {
    const command = formOf({ ...AGENT, action: { type: 'command', command: 'ls', timeoutSecs: 30 } });
    const script = formOf({ ...AGENT, action: { type: 'script', name: 'disk-hogs', scope: 'repo' } });

    expect(command).toMatchObject({ kind: 'command', command: 'ls', timeout: '30' });
    expect(script).toMatchObject({ kind: 'script', scriptName: 'disk-hogs', scriptScope: 'repo' });
    expect(draftOf(script, null).action).toEqual({ type: 'script', name: 'disk-hogs', scope: 'repo' });
  });
});

const PROJECTS: ProjectScope[] = [
  { path: '/work/other', name: 'other', root: '/work/other', key: 'git:github.com/acme/other' },
  { path: '/work/app/packages/ui', name: 'ui', root: '/work/app', key: 'git:github.com/acme/app' },
];

describe('the project a routine is for', () => {
  it('is the project of the root the routine names, not the open one', () => {
    const form = formOf(AGENT, PROJECTS);

    expect(form.projectPath).toBe('/work/app/packages/ui');
    expect(targetOf(form)).toEqual({ kind: 'repo', path: '/work/app/packages/ui' });
    expect(draftOf(form, { id: AGENT.id, sha256: 'f'.repeat(64) })).toMatchObject({ scope: 'project', project: '/work/app/packages/ui' });
  });

  it('is the root itself while no project of the list has it, so an edit still names the folder the file has', () => {
    expect(formOf(AGENT, [PROJECTS[0]]).projectPath).toBe('/work/app');
    expect(formOf({ ...AGENT, scope: 'global', project: null }, PROJECTS)).toMatchObject({ scope: 'global', projectPath: null });
  });

  it('follows the target chosen in the editor', () => {
    const form = withTarget(complete(), { kind: 'repo', path: '/work/other' });

    expect(form).toMatchObject({ scope: 'project', projectPath: '/work/other' });
    expect(draftOf(form, null)).toMatchObject({ scope: 'project', project: '/work/other' });
    expect(targetOf(blankForm({ kind: 'repo', path: '/work/other' }))).toEqual({ kind: 'repo', path: '/work/other' });
    expect(targetOf(blankForm())).toEqual({ kind: 'global' });
  });
});

describe('the draft', () => {
  it('trims what was typed, leaves out what was left empty, and keeps the project only for a project routine', () => {
    const draft = draftOf(complete({ name: '  Archive logs ', description: '   ', prompt: ' Pack up. ', maxMinutes: '', scope: 'global', projectPath: null }), null);

    expect(draft).toMatchObject({ id: null, expectedSha256: null, name: 'Archive logs', description: null, scope: 'global', project: null });
    expect(draft.action).toEqual({ type: 'agent', harness: 'omp', model: null, prompt: 'Pack up.', approvalMode: null, trustOnce: false, maxMinutes: null });
    expect(draftOf(complete({ scope: 'project', projectPath: '/work/app' }), null).project).toBe('/work/app');
  });

  it('sends a model only when both its parts are there, and an approval mode or a trust only to the harness that has it', () => {
    expect(draftOf(complete({ provider: 'a', modelId: 'b' }), null).action).toMatchObject({ model: { provider: 'a', id: 'b' } });
    expect(draftOf(withHarness(complete({ approvalMode: 'yolo', trustOnce: true }), 'pi'), null).action).toMatchObject({ harness: 'pi', approvalMode: null, trustOnce: true });
    expect(draftOf(withHarness(complete({ harness: 'pi', trustOnce: true }), 'omp'), null).action).toMatchObject({ harness: 'omp', approvalMode: null, trustOnce: false });
  });

  it('sends an approval mode for every harness that has approval modes and a one-time trust only for one with project trust', () => {
    const opencode = withHarness(complete({ approvalMode: 'write', trustOnce: true }), 'opencode');

    expect(draftOf(opencode, null).action).toMatchObject({ harness: 'opencode', approvalMode: 'write', trustOnce: false });
    expect(runsUnattended({ ...opencode, approvalMode: 'yolo' })).toBe(true);
    expect(runsUnattended({ ...opencode, approvalMode: 'write' })).toBe(false);
  });

  it('drops Yolo for a harness without it and keeps it where it is offered', () => {
    harness.summaries = fixtures.harnesses.map((summary) =>
      summary.info.id === 'claude' ? { ...summary, capabilities: { ...summary.capabilities, chat: true } } : summary,
    );

    const claude = withHarness(complete({ approvalMode: 'yolo' }), 'claude');
    expect(claude.approvalMode).toBeNull();
    expect(draftOf({ ...claude, approvalMode: 'yolo' }, null).action).toMatchObject({ harness: 'claude', approvalMode: null });
    expect(withHarness(complete({ approvalMode: 'write' }), 'claude').approvalMode).toBe('write');
    expect(runsUnattended({ ...claude, approvalMode: 'yolo' })).toBe(false);

    expect(withHarness(complete({ approvalMode: 'yolo' }), 'omp').approvalMode).toBe('yolo');
    expect(runsUnattended(complete({ approvalMode: 'yolo' }))).toBe(true);
  });

  it('keeps Auto only for Claude Code, where it runs tools unattended', () => {
    harness.summaries = fixtures.harnesses.map((summary) =>
      summary.info.id === 'claude' ? { ...summary, capabilities: { ...summary.capabilities, chat: true } } : summary,
    );

    const claude = withHarness(complete({ approvalMode: 'auto' }), 'claude');
    expect(claude.approvalMode).toBe('auto');
    expect(draftOf(claude, null).action).toMatchObject({ harness: 'claude', approvalMode: 'auto' });
    expect(runsUnattended(claude)).toBe(true);

    for (const other of ['omp', 'opencode', 'pi'] as const) {
      const moved = withHarness(claude, other);
      expect(moved.approvalMode, other).toBeNull();
      expect(runsUnattended({ ...moved, approvalMode: 'auto' }), other).toBe(false);
    }
  });

  it('follows the capabilities the harness list reports and not the name of the harness', () => {
    harness.summaries = fixtures.harnesses.map((summary) =>
      summary.info.id === 'omp' ? { ...summary, capabilities: { ...summary.capabilities, approvalModes: false, projectTrust: true } } : summary,
    );

    expect(draftOf(complete({ approvalMode: 'yolo', trustOnce: true }), null).action).toMatchObject({ harness: 'omp', approvalMode: null, trustOnce: true });
    expect(runsUnattended(complete({ approvalMode: null, trustOnce: true }))).toBe(true);
  });

  it('keeps what a routine holds for a harness the list does not know yet, so nothing is lost on a save', () => {
    harness.summaries = [];

    expect(draftOf(complete({ approvalMode: 'write' }), null).action).toMatchObject({ approvalMode: 'write' });
  });

  it('turns a global routine\'s repository script into a global one, since it has no repository', () => {
    const form = withTarget(complete({ kind: 'script', scriptName: 'disk-hogs', scriptScope: 'repo', scope: 'project', projectPath: '/work/app' }), { kind: 'global' });

    expect(form).toMatchObject({ scope: 'global', projectPath: null, scriptScope: 'global' });
  });

  it('writes a switch as the routine with only its schedule turned on or off', () => {
    const view = { id: AGENT.id, sha256: 'f'.repeat(64) } as Parameters<typeof toggleDraft>[0];

    expect(toggleDraft(view, AGENT, false)).toEqual({ id: AGENT.id, expectedSha256: 'f'.repeat(64), name: AGENT.name, description: AGENT.description, scope: 'project', project: '/work/app', when: AGENT.when, action: AGENT.action, enabled: false });
  });

  it('refuses to write a schedule that is not complete', () => {
    expect(() => draftOf(complete({ whenKind: 'cron', cron: '' }), null)).toThrow('the schedule is not valid');
  });
});

describe('the schedule', () => {
  it('is complete only when its fields are acceptable', () => {
    expect(whenOf(complete({ whenKind: 'daily', at: '09:00' }))).toEqual({ type: 'daily', at: '09:00' });
    expect(whenOf(complete({ whenKind: 'daily', at: '24:00' }))).toBeNull();
    expect(whenOf(complete({ whenKind: 'daily', at: '' }))).toBeNull();
    expect(whenOf(complete({ whenKind: 'everyMinutes', every: '720' }))).toEqual({ type: 'everyMinutes', every: 720 });
    expect(whenOf(complete({ whenKind: 'everyMinutes', every: '721' }))).toBeNull();
    expect(whenOf(complete({ whenKind: 'everyHours', every: '25' }))).toBeNull();
    expect(whenOf(complete({ whenKind: 'everyHours', every: '1.5' }))).toBeNull();
    expect(whenOf(complete({ whenKind: 'cron', cron: '  */5 * * * * ' }))).toEqual({ type: 'cron', expression: '*/5 * * * *' });
    expect(whenOf(complete({ whenKind: 'cron', cron: '' }))).toBeNull();
    expect(whenOf(complete({ whenKind: 'appStart' }))).toEqual({ type: 'appStart' });
  });
});

describe('what keeps a routine from being saved', () => {
  it('accepts a routine that is complete', () => {
    expect(problemsOf(complete(), CONTEXT)).toEqual([]);
    expect(problemsOf(complete({ kind: 'command', command: 'ls' }), CONTEXT)).toEqual([]);
    expect(problemsOf(complete({ kind: 'script', scriptName: 'disk-hogs' }), CONTEXT)).toEqual([]);
  });

  it('wants a name and the text of the action, and says so only once the user has been at the field', () => {
    const found = problemsOf(blankForm(), CONTEXT);

    expect(found.map((problem) => [problem.field, problem.required])).toEqual([['name', true], ['prompt', true]]);
    expect(fieldsWithProblems(complete({ kind: 'command', command: '  ' }))).toEqual(['command']);
    expect(fieldsWithProblems(complete({ kind: 'script', scriptName: '' }))).toEqual(['script']);
  });

  it('counts the characters of a name, a description, a prompt and a command', () => {
    expect(fieldsWithProblems(complete({ name: 'x'.repeat(81) }))).toEqual(['name']);
    expect(fieldsWithProblems(complete({ name: 'x'.repeat(80), description: 'y'.repeat(501) }))).toEqual(['description']);
    expect(fieldsWithProblems(complete({ prompt: 'z'.repeat(8193) }))).toEqual(['prompt']);
    expect(fieldsWithProblems(complete({ kind: 'command', command: 'c'.repeat(2049) }))).toEqual(['command']);
    // The format counts code points, not UTF-16 units: 80 emoji fit.
    expect(fieldsWithProblems(complete({ name: '😀'.repeat(80) }))).toEqual([]);
  });

  it('refuses control characters in a name and hidden characters in a prompt or command, but not line breaks', () => {
    expect(fieldsWithProblems(complete({ name: 'Back\u0007up' }))).toEqual(['name']);
    expect(fieldsWithProblems(complete({ prompt: 'Say‮ hello' }))).toEqual(['prompt']);
    expect(fieldsWithProblems(complete({ kind: 'command', command: 'ls​' }))).toEqual(['command']);
    expect(fieldsWithProblems(complete({ prompt: 'Line one\nLine two\twith a tab' }))).toEqual([]);
    expect(hasHiddenCharacters('plain\r\ntext')).toBe(false);
    expect(hasHiddenCharacters('hidden​text')).toBe(true);
  });

  it('holds the numbers to the ranges of the format', () => {
    expect(fieldsWithProblems(complete({ maxMinutes: '241' }))).toEqual(['maxMinutes']);
    expect(fieldsWithProblems(complete({ maxMinutes: '0' }))).toEqual(['maxMinutes']);
    expect(fieldsWithProblems(complete({ maxMinutes: '240' }))).toEqual([]);
    expect(fieldsWithProblems(complete({ kind: 'command', command: 'ls', timeout: '3601' }))).toEqual(['timeout']);
    expect(fieldsWithProblems(complete({ whenKind: 'daily', at: '9:00' }))).toEqual(['at']);
    expect(fieldsWithProblems(complete({ whenKind: 'everyHours', every: '0' }))).toEqual(['every']);
    expect(fieldsWithProblems(complete({ whenKind: 'cron', cron: 'c'.repeat(101) }))).toEqual(['cron']);
    expect(fieldsWithProblems(complete({ whenKind: 'cron', cron: '' }))).toEqual(['cron']);
  });

  it('wants both parts of a model or neither', () => {
    expect(fieldsWithProblems(complete({ provider: 'anthropic' }))).toEqual(['model']);
    expect(fieldsWithProblems(complete({ modelId: 'x' }))).toEqual(['model']);
    expect(fieldsWithProblems(complete({ provider: 'p'.repeat(201), modelId: 'x' }))).toEqual(['model']);
  });

  it('wants a script name in the form harness memory keeps, and a repository only for a project routine', () => {
    expect(fieldsWithProblems(complete({ kind: 'script', scriptName: 'Disk Hogs' }))).toEqual(['script']);
    expect(fieldsWithProblems(complete({ kind: 'script', scriptName: 'a'.repeat(65) }))).toEqual(['script']);
    expect(fieldsWithProblems(complete({ kind: 'script', scriptName: 'disk-hogs', scriptScope: 'repo', scope: 'global' }))).toEqual(['script']);
    expect(fieldsWithProblems(complete({ kind: 'script', scriptName: 'disk-hogs', scriptScope: 'repo', scope: 'project' }))).toEqual([]);
  });

  it('wants a chosen project for a project routine, and room in the scope', () => {
    expect(fieldsWithProblems(complete({ scope: 'project' }), { projectChosen: false, scopeFull: false })).toEqual(['scope']);
    expect(fieldsWithProblems(complete(), { projectChosen: true, scopeFull: true })).toEqual(['scope']);
  });
});

describe('a routine that runs tools without asking', () => {
  it('is an OMP agent set to yolo, or a PI agent that trusts the project once, and nothing else', () => {
    expect(runsUnattended(complete({ approvalMode: 'yolo' }))).toBe(true);
    expect(runsUnattended(complete({ approvalMode: 'write' }))).toBe(false);
    expect(runsUnattended(complete({ approvalMode: null }))).toBe(false);
    expect(runsUnattended(complete({ harness: 'pi', trustOnce: true }))).toBe(true);
    expect(runsUnattended(complete({ harness: 'pi', trustOnce: false }))).toBe(false);
    expect(runsUnattended(complete({ kind: 'command', approvalMode: 'yolo' }))).toBe(false);
  });
});
