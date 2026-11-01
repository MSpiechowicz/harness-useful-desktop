import { describe, expect, it } from 'vitest';

import { choosesPlace, effectivePlace, placesFor, REMOTE_ROUTING, usesBothPlaces, type DocumentRouting } from './knowledge-routing';

const both: DocumentRouting = { repositoryDocuments: 'remoteLocal', globalDocuments: 'remote' };

describe('which places documents are kept in', () => {
  it('uses one place without a remote database, whatever the routing says', () => {
    expect(usesBothPlaces(false, both)).toBe(false);
  });

  it('uses both places for remote + this computer anywhere and for a split between the places, never for one place', () => {
    expect(usesBothPlaces(true, REMOTE_ROUTING)).toBe(false);
    expect(usesBothPlaces(true, { repositoryDocuments: 'local', globalDocuments: 'local' })).toBe(false);
    expect(usesBothPlaces(true, both)).toBe(true);
    expect(usesBothPlaces(true, { repositoryDocuments: 'remote', globalDocuments: 'local' })).toBe(true);
    expect(usesBothPlaces(true, { repositoryDocuments: 'local', globalDocuments: 'remote' })).toBe(true);
  });

  it('offers the choice of a place only while a kind reads remote + this computer', () => {
    expect(choosesPlace(true, both)).toBe(true);
    expect(choosesPlace(true, { repositoryDocuments: 'remote', globalDocuments: 'local' })).toBe(false);
    expect(choosesPlace(false, both)).toBe(false);
  });

  it('allows what the routing of the kind allows and defaults to remote unless it is local only', () => {
    expect(placesFor('remote')).toEqual({ allowed: ['remote'], fallback: 'remote' });
    expect(placesFor('local')).toEqual({ allowed: ['local'], fallback: 'local' });
    expect(placesFor('remoteLocal')).toEqual({ allowed: ['remote', 'local'], fallback: 'remote' });
  });

  it('keeps the choice for a kind that allows it and falls back to the default for one that does not', () => {
    expect(effectivePlace(both, false, 'local')).toBe('local');
    expect(effectivePlace(both, false, null)).toBe('remote');
    expect(effectivePlace(both, true, 'local')).toBe('remote');
    expect(effectivePlace({ repositoryDocuments: 'remoteLocal', globalDocuments: 'local' }, true, 'remote')).toBe('local');
  });
});
