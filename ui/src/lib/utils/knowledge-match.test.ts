import { describe, expect, it } from 'vitest';

import type { KnowledgeIntake } from '$lib/ipc/bindings/KnowledgeIntake';
import type { KnowledgeSimilar } from '$lib/ipc/bindings/KnowledgeSimilar';

import { matchRows } from './knowledge-match';

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
