import { describe, expect, it } from 'vitest';

import type { UpdateInfo } from '$lib/ipc/bindings/UpdateInfo';

import { releaseNotes } from './copy';

function notes(text: string | null, version = '0.2.0'): string {
  const info: UpdateInfo = { version, currentVersion: '0.1.0', date: null, notes: text };

  return releaseNotes(info);
}

describe('releaseNotes', () => {
  it.each([
    ['a plain version heading', '## 0.2.0\n\n* faster'],
    ['a heading with the date', '## 0.2.0 (2026-10-08)\n\n* faster'],
    ['a linked heading as semantic-release writes it', '# [0.2.0](https://example.test/compare/v0.1.0...v0.2.0) (2026-10-08)\n\n* faster'],
    ['blank lines before the heading', '\n\n## 0.2.0\n\n* faster'],
  ])('drops %s', (_name, text) => {
    expect(notes(text)).toBe('* faster');
  });

  it('keeps a heading that does not announce this version', () => {
    expect(notes('## Features\n\n* faster')).toBe('## Features\n\n* faster');
    expect(notes('## 0.1.9\n\n* faster')).toBe('## 0.1.9\n\n* faster');
  });

  it('only looks at the first block, so later headings stay', () => {
    expect(notes('Intro\n\n## 0.2.0\n\n* faster')).toBe('Intro\n\n## 0.2.0\n\n* faster');
  });

  it('is empty when nothing is left or there are no notes', () => {
    expect(notes('## 0.2.0')).toBe('');
    expect(notes(null)).toBe('');
  });
});
