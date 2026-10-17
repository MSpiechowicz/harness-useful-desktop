import { describe, expect, it } from 'vitest';

import type { Hook } from '$lib/ipc/bindings/Hook';

import { blankForm, draftOf, formOf, type FormContext, LIMITS, matcherProblem, problemsOf, validActions, withEvent } from './hook-form';

const OPEN: FormContext = { projectOpen: true, scopeFull: false };

function valid() {
  return { ...blankForm(), name: 'Check', text: 'Look at the notes.' };
}

describe('the actions a hook can take', () => {
  it.each([
    ['sessionStart', ['addContext', 'command']],
    ['promptSubmit', ['addContext', 'command']],
    ['toolEnd', ['followUp', 'command']],
    ['turnEnd', ['followUp', 'command']],
    ['sessionEnd', ['command']],
  ] as const)('are limited at %s', (event, actions) => {
    expect(validActions(event)).toEqual(actions);
  });

  it('keeps the action when the new event allows it, and takes the first allowed one when not', () => {
    const command = { ...valid(), kind: 'command' as const };
    expect(withEvent(command, 'sessionEnd').kind).toBe('command');
    expect(withEvent(command, 'toolEnd').kind).toBe('command');

    expect(withEvent(valid(), 'promptSubmit').kind).toBe('addContext');
    expect(withEvent(valid(), 'turnEnd').kind).toBe('followUp');
    expect(withEvent(valid(), 'sessionEnd').kind).toBe('command');
  });

  it('keeps what was typed for the action that is not on screen', () => {
    const typed = { ...valid(), text: 'Remember the notes.', command: 'bun test' };

    expect(withEvent(withEvent(typed, 'sessionEnd'), 'sessionStart')).toMatchObject({ kind: 'command', text: 'Remember the notes.', command: 'bun test' });
  });
});

describe('the tool filter', () => {
  it.each(['', '  ', 'edit', 'edit|write', 'EDIT|Write', 'mcp__server.tool:name-1', 'a'.repeat(LIMITS.matcher)])('accepts %j', (matcher) => {
    expect(matcherProblem(matcher)).toBeNull();
  });

  it.each([
    ['edit | write', 'chars'],
    ['edit write', 'chars'],
    ['edit*', 'chars'],
    ['(edit)', 'chars'],
    ['edit|', 'empty'],
    ['|edit', 'empty'],
    ['edit||write', 'empty'],
    ['|', 'empty'],
    ['a'.repeat(LIMITS.matcher + 1), 'long'],
  ] as const)('refuses %j as %s', (matcher, problem) => {
    expect(matcherProblem(matcher)).toBe(problem);
  });

  it('only matters at the tool event', () => {
    const form = { ...valid(), matcher: 'edit | write' };

    expect(problemsOf(form, OPEN)).toEqual([]);
    expect(problemsOf(withEvent(form, 'toolEnd'), OPEN).map((problem) => problem.field)).toEqual(['matcher']);
  });

  it('is left out of the draft for any other event', () => {
    const form = { ...valid(), matcher: 'edit' };

    expect(draftOf(form, null, null).matcher).toBeNull();
    expect(draftOf(withEvent(form, 'toolEnd'), null, null).matcher).toBe('edit');
  });
});

describe('the fields', () => {
  it('want a name and a text before the hook can be saved', () => {
    const problems = problemsOf(blankForm(), OPEN);

    expect(problems.map((problem) => [problem.field, problem.required])).toEqual([
      ['name', true],
      ['text', true],
    ]);
    expect(problemsOf(valid(), OPEN)).toEqual([]);
  });

  it('count characters the way the format does, in code points', () => {
    const emoji = '🙂';

    expect(problemsOf({ ...valid(), name: emoji.repeat(LIMITS.name) }, OPEN)).toEqual([]);
    expect(problemsOf({ ...valid(), name: emoji.repeat(LIMITS.name + 1) }, OPEN).map((problem) => problem.message)).toEqual(['hooks.editor.name.long']);
  });

  it('refuse a name with control characters, and a text that is too long', () => {
    expect(problemsOf({ ...valid(), name: 'two\nlines' }, OPEN)[0].message).toBe('hooks.editor.name.control');
    expect(problemsOf({ ...valid(), text: 'x'.repeat(LIMITS.text + 1) }, OPEN)[0].message).toBe('hooks.editor.text.long');
    expect(problemsOf({ ...valid(), text: 'x'.repeat(LIMITS.text) }, OPEN)).toEqual([]);
    expect(problemsOf({ ...valid(), description: 'x'.repeat(LIMITS.description + 1) }, OPEN)[0].field).toBe('description');
  });

  it('check the command and its timeout when the action is a command', () => {
    const command = { ...valid(), kind: 'command' as const, text: '' };

    expect(problemsOf(command, OPEN).map((problem) => problem.field)).toEqual(['command']);
    expect(problemsOf({ ...command, command: 'bun test' }, OPEN)).toEqual([]);

    for (const timeout of ['', '1', '10', '30']) {
      expect(problemsOf({ ...command, command: 'bun test', timeout }, OPEN)).toEqual([]);
    }

    for (const timeout of ['0', '31', '1.5', '-3', 'ten']) {
      expect(problemsOf({ ...command, command: 'bun test', timeout }, OPEN).map((problem) => problem.field)).toEqual(['timeout']);
    }
  });

  it('ignore a command that is not the action', () => {
    expect(problemsOf({ ...valid(), command: 'x'.repeat(LIMITS.command + 10), timeout: '99' }, OPEN)).toEqual([]);
  });

  it('need an open project for a project hook, and room in a full scope', () => {
    const project = { ...valid(), scope: 'project' as const };

    expect(problemsOf(project, { projectOpen: false, scopeFull: false }).map((problem) => problem.message)).toEqual(['hooks.editor.scope.noProject']);
    expect(problemsOf(project, OPEN)).toEqual([]);
    expect(problemsOf(valid(), { projectOpen: true, scopeFull: true }).map((problem) => problem.message)).toEqual(['hooks.editor.scope.full']);
  });
});

describe('the draft', () => {
  it('trims the text and names the file and version of an edit', () => {
    const form = { ...valid(), name: '  Check  ', description: '  ', text: '  Look.  ', scope: 'project' as const };

    expect(draftOf(form, { id: 'a'.repeat(32), sha256: 'b'.repeat(64) }, '/work/app')).toEqual({
      id: 'a'.repeat(32),
      expectedSha256: 'b'.repeat(64),
      name: 'Check',
      description: null,
      scope: 'project',
      project: '/work/app',
      event: 'sessionStart',
      matcher: null,
      action: { type: 'addContext', text: 'Look.' },
      enabled: true,
    });
  });

  it('sends no project for a global hook and the timeout of a command only when one was typed', () => {
    const command = { ...valid(), kind: 'command' as const, command: 'bun test ', event: 'sessionEnd' as const };

    expect(draftOf(command, null, '/work/app')).toMatchObject({ project: null, id: null, expectedSha256: null, action: { type: 'command', command: 'bun test', timeoutSecs: null } });
    expect(draftOf({ ...command, timeout: ' 20 ' }, null, null).action).toEqual({ type: 'command', command: 'bun test', timeoutSecs: 20 });
  });

  it('is the form of a hook again after editing it', () => {
    const hook: Hook = {
      id: 'a'.repeat(32),
      name: 'Lint',
      description: 'Style',
      scope: 'project',
      project: '/work/app',
      event: 'toolEnd',
      matcher: 'edit|write',
      action: { type: 'command', command: 'bun run lint', timeoutSecs: 20 },
      enabled: false,
      requires: [],
      seed: null,
      createdAt: '2026-10-17T10:00:00Z',
      createdBy: 'agent',
      updatedAt: '2026-10-17T10:00:00Z',
      updatedBy: 'agent',
    };

    const form = formOf(hook);

    expect(form).toMatchObject({ name: 'Lint', description: 'Style', kind: 'command', command: 'bun run lint', timeout: '20', matcher: 'edit|write', scope: 'project', enabled: false });
    expect(draftOf(form, { id: hook.id, sha256: 'c'.repeat(64) }, '/work/app')).toMatchObject({ name: 'Lint', matcher: 'edit|write', action: hook.action, enabled: false, project: '/work/app' });
  });
});
