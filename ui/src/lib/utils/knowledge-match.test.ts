import { describe, expect, it } from 'vitest';

import type { KnowledgeIntake } from '$lib/ipc/bindings/KnowledgeIntake';
import type { KnowledgeSimilar } from '$lib/ipc/bindings/KnowledgeSimilar';

import { goesToTeam, matchRows } from './knowledge-match';

const FILE = '/home/ada/docs/guide/install.md';
const OTHER = '/home/ada/docs/guide/faq.md';

const intake: KnowledgeIntake = {
  token: 'intake-1',
  files: [{ name: 'install.md', path: FILE, bytes: 120 }],
  folders: [{ name: 'guide', path: '/home/ada/docs/guide', bytes: null }],
  skipped: [],
};

const match = { id: 7, title: 'Install', place: 'remote', reason: 'sourceKey', sharedChunks: 3, chunks: 3, updatedAt: '2026-11-01T10:00:00Z' } as const;

describe('matchRows', () => {
  it('shows a file once when the selection holds it and the folder it is in', () => {
    // The folder expands to the same file, so the comparison answers for its path twice.
    const similar: KnowledgeSimilar = { items: [{ path: FILE, match }, { path: FILE, match }, { path: OTHER, match: null }], offline: false };

    const rows = matchRows(intake, similar);

    expect(rows.map((row) => row.path)).toEqual([FILE, OTHER]);
    expect(rows[0]).toMatchObject({ fromFolder: false, replaceable: true });
  });
});

describe('goesToTeam', () => {
  const similar = (place: 'local' | 'remote'): KnowledgeSimilar => ({ items: [{ path: FILE, match: { ...match, place } }, { path: OTHER, match: null }], offline: false });
  const rows = (place: 'local' | 'remote') => matchRows(intake, similar(place));

  it('sends a new file to the place the import files new documents in', () => {
    expect(goesToTeam(null, {}, 'one', 'remote')).toBe(true);
    expect(goesToTeam(null, {}, 'one', 'local')).toBe(false);
    expect(goesToTeam([], {}, 'one', 'local')).toBe(false);
    expect(goesToTeam(rows('local'), { [`${FILE}|7`]: false }, 'one', 'remote')).toBe(true);
  });

  it('sends a replacement to the place of the document it replaces, not the place of the new files', () => {
    // Only the matched file is staged, and it updates a document kept on this computer while new files would go to the team.
    const only = matchRows(intake, { items: [{ path: FILE, match: { ...match, place: 'local' } }], offline: false });

    expect(goesToTeam(only, {}, 'one', 'remote')).toBe(false);
    expect(goesToTeam(only, { [`${FILE}|7`]: false }, 'one', 'remote')).toBe(true);
  });

  it('leaves out a file that could not be compared: it is not imported', () => {
    const failed = matchRows(intake, { items: [{ path: OTHER, match: null, error: 'unreadable' }], offline: false });

    expect(goesToTeam(failed, {}, 'one', 'remote')).toBe(false);
  });

  it('is true as soon as one file of several goes to the team', () => {
    expect(goesToTeam(rows('local'), {}, 'one', 'remote')).toBe(true);
    expect(goesToTeam(rows('remote'), {}, 'one', 'local')).toBe(true);
    expect(goesToTeam(rows('local'), {}, 'one', 'local')).toBe(false);
  });
});
