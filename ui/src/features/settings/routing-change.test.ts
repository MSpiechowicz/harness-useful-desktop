import { describe, expect, it } from 'vitest';

import type { MemoryNotesRouting } from '$lib/ipc/bindings/MemoryNotesRouting';

import { copyOffers, readsRemote, routesChanged, routingEffects, scopesMovedToTeam, type LocalHoldings, type Routes } from './routing-change';

const HOLDINGS: LocalHoldings = { notes: { repo: 7, global: 2 }, scripts: 5 };

function allAt(place: MemoryNotesRouting): Routes {
  return { repositoryNotes: place, globalNotes: place, scripts: place };
}

/** Everything at `from`, then the repository notes at `to`: the other scopes stay put. */
function repositoryMove(from: MemoryNotesRouting, to: MemoryNotesRouting, holdings = HOLDINGS) {
  return routingEffects(allAt(from), { ...allAt(from), repositoryNotes: to }, holdings);
}

/** The same for the scripts. */
function scriptsMove(from: MemoryNotesRouting, to: MemoryNotesRouting, holdings = HOLDINGS) {
  return routingEffects(allAt(from), { ...allAt(from), scripts: to }, holdings);
}

describe('What a change of where notes are kept does', () => {
  it.each([
    ['remote', 'local', 'hideTeam', null],
    ['remote', 'remoteLocal', 'saveHere', null],
    ['local', 'remote', 'hideLocal', 7],
    ['remoteLocal', 'remote', 'hideLocal', 7],
    ['local', 'remoteLocal', 'showTeam', null],
    ['remoteLocal', 'local', 'hideTeam', null],
  ] as const)('%s to %s is one %s effect on the scope that moved', (from, to, kind, count) => {
    expect(repositoryMove(from, to)).toEqual([{ scope: 'repository', kind, count }]);
  });

  it('says nothing about a scope that stays where it is', () => {
    expect(repositoryMove('local', 'local')).toEqual([]);
    expect(repositoryMove('remoteLocal', 'remoteLocal')).toEqual([]);
  });

  it('lists the scopes in the order of the form, each with its own count', () => {
    const effects = routingEffects({ repositoryNotes: 'local', globalNotes: 'remoteLocal', scripts: 'local' }, allAt('remote'), HOLDINGS);

    expect(effects).toEqual([
      { scope: 'repository', kind: 'hideLocal', count: 7 },
      { scope: 'global', kind: 'hideLocal', count: 2 },
      { scope: 'scripts', kind: 'hideLocal', count: 5 },
    ]);
  });

  it('says nothing about moving to the remote database when nothing is saved on this computer', () => {
    expect(repositoryMove('local', 'remote', { notes: { repo: 0, global: 4 }, scripts: 3 })).toEqual([]);
  });

  it('still warns when what is saved on this computer cannot be counted', () => {
    const unknown: LocalHoldings = { notes: null, scripts: null };

    expect(repositoryMove('local', 'remote', unknown)).toEqual([{ scope: 'repository', kind: 'hideLocal', count: null }]);
    expect(scriptsMove('local', 'remote', unknown)).toEqual([{ scope: 'scripts', kind: 'hideLocal', count: null }]);
  });
});

describe('What a change of where scripts are kept does', () => {
  it.each([
    ['remote', 'local', 'hideTeam', null],
    ['remote', 'remoteLocal', 'saveHere', null],
    ['local', 'remote', 'hideLocal', 5],
    ['remoteLocal', 'remote', 'hideLocal', 5],
    ['local', 'remoteLocal', 'showTeam', null],
    ['remoteLocal', 'local', 'hideTeam', null],
  ] as const)('%s to %s is one %s effect on the scripts alone', (from, to, kind, count) => {
    expect(scriptsMove(from, to)).toEqual([{ scope: 'scripts', kind, count }]);
  });

  it('counts the scripts, not the notes, of this computer', () => {
    expect(scriptsMove('local', 'remote', { notes: { repo: 9, global: 9 }, scripts: 1 })).toEqual([{ scope: 'scripts', kind: 'hideLocal', count: 1 }]);
    expect(scriptsMove('local', 'remote', { notes: { repo: 9, global: 9 }, scripts: 0 })).toEqual([]);
  });
});

describe('Whether the form changes where things are kept', () => {
  it('compares the notes and the scripts', () => {
    const saved = allAt('remote');

    expect(routesChanged(saved, { ...saved })).toBe(false);
    expect(routesChanged(saved, { ...saved, globalNotes: 'remoteLocal' })).toBe(true);
    expect(routesChanged(saved, { ...saved, scripts: 'local' })).toBe(true);
  });

  it('reads the remote database in every place but This computer', () => {
    expect(['remote', 'local', 'remoteLocal'].map((place) => readsRemote(place as MemoryNotesRouting))).toEqual([true, false, true]);
  });
});

describe('What to offer to copy after a save', () => {
  const before: Routes = { repositoryNotes: 'local', globalNotes: 'remoteLocal', scripts: 'local' };

  it('is the scopes that moved to the remote database alone', () => {
    expect(scopesMovedToTeam(before, true, { repositoryNotes: 'remote', globalNotes: 'remoteLocal', scripts: 'remote' })).toEqual(['repository', 'scripts']);
  });

  it('leaves out a scope that was in the remote database already', () => {
    expect(scopesMovedToTeam({ ...allAt('remote'), globalNotes: 'local' }, true, allAt('remote'))).toEqual(['global']);
  });

  it('is every scope that is remote on a first connection, where everything was local', () => {
    expect(scopesMovedToTeam(allAt('remote'), false, { ...allAt('remote'), globalNotes: 'local' })).toEqual(['repository', 'scripts']);
  });

  it('asks about the notes first and the scripts second, each only when this computer holds some', () => {
    expect(copyOffers(['repository', 'scripts'], HOLDINGS)).toEqual(['notes', 'scripts']);
    expect(copyOffers(['scripts'], HOLDINGS)).toEqual(['scripts']);
    expect(copyOffers(['global', 'scripts'], { notes: { repo: 4, global: 0 }, scripts: 5 })).toEqual(['scripts']);
    expect(copyOffers(['repository', 'scripts'], { notes: { repo: 4, global: 0 }, scripts: 0 })).toEqual(['notes']);
  });

  it('asks about nothing when nothing moved or what is saved here cannot be counted', () => {
    expect(copyOffers([], HOLDINGS)).toEqual([]);
    expect(copyOffers(['repository', 'global', 'scripts'], { notes: null, scripts: null })).toEqual([]);
  });
});
