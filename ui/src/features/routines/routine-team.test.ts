import { describe, expect, it } from 'vitest';

import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';
import type { RoutineView } from '$lib/ipc/bindings/RoutineView';

import { switchLock, teamProject } from './routine-team';

const PROJECTS: ProjectScope[] = [
  { path: '/work/app/packages/ui', name: 'ui', root: '/work/app', key: 'git:github.com/acme/app' },
  { path: '/work/notes', name: 'notes', root: '/work/notes', key: null },
];

function team(patch: Partial<RoutineView>): RoutineView {
  return { id: 'a'.repeat(32), origin: 'team', routine: { scope: 'project' }, status: 'off', teamOn: false, teamRepo: null, repoKey: null, root: null, ...patch } as RoutineView;
}

describe('the folder a remote routine is turned on and copied in', () => {
  it('is the project that has the repository of the routine, found by key', () => {
    expect(teamProject(team({ teamRepo: 'git:github.com/acme/app' }), PROJECTS)).toBe('/work/app/packages/ui');
  });

  it('is the project with the root a refresh confirmed when no project has the key', () => {
    expect(teamProject(team({ teamRepo: 'git:github.com/acme/other', root: '/work/notes' }), PROJECTS)).toBe('/work/notes');
  });

  it('is the confirmed root itself when no project of the list has it, and none for a repository this computer lacks', () => {
    expect(teamProject(team({ teamRepo: 'git:github.com/acme/other', root: '/work/gone' }), PROJECTS)).toBe('/work/gone');
    expect(teamProject(team({ teamRepo: 'git:github.com/acme/website' }), PROJECTS)).toBeNull();
  });

  it('is none for a global routine, which needs no folder', () => {
    expect(teamProject(team({ routine: { scope: 'global' } as RoutineView['routine'] }), PROJECTS)).toBeNull();
  });
});

describe('the lock on the switch of a remote routine', () => {
  it('holds turning on a routine of a repository while no folder of it is known, and lets turning off and global routines through', () => {
    const repository = team({ teamRepo: 'git:github.com/acme/website' });

    expect(switchLock(repository, false)).toBe('routines.locked.teamNoProject');
    expect(switchLock(repository, true)).toBeNull();
    expect(switchLock({ ...repository, teamOn: true }, false)).toBeNull();
    expect(switchLock(team({}), false)).toBeNull();
  });
});
