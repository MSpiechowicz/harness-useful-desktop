import { describe, expect, it } from 'vitest';

import { describeEffect, pieces } from './setup-effects';

const SIDE_EFFECTS = [
  'Installs the Harness Dashboard app to ~/.local/lib/harness-dashboard and the harness-dashboard command to ~/.local/bin.',
  'Adds Harness Dashboard to your applications menu.',
  'Appends a line that puts ~/.local/bin on your PATH to your shell profile, if it is not on PATH yet.',
  'Stops a running dashboard server or app so the new version can start; `harness-dashboard mcp` keeps running.',
];

describe('describeEffect', () => {
  it('names what each installer side effect is about', () => {
    expect(SIDE_EFFECTS.map((text) => describeEffect(text).kind)).toEqual(['install', 'menu', 'path', 'stop']);
  });

  it('keeps a sentence it does not know whole, under a neutral kind', () => {
    expect(describeEffect('Does something nobody planned for.')).toEqual({ kind: 'other', title: 'Does something nobody planned for.', detail: '' });
  });

  it('splits a sentence into a headline and an aside without losing any of it', () => {
    const split = describeEffect(SIDE_EFFECTS[3]!);

    expect(split.title).toBe('Stops a running dashboard server or app so the new version can start');
    expect(split.detail).toBe('`harness-dashboard mcp` keeps running.');

    expect(describeEffect(SIDE_EFFECTS[2]!).detail).toBe('If it is not on PATH yet.');
  });
});

describe('pieces', () => {
  it('sets paths and `commands` apart from the words around them', () => {
    expect(pieces(SIDE_EFFECTS[0]!).filter((piece) => piece.code).map((piece) => piece.text)).toEqual(['~/.local/lib/harness-dashboard', '~/.local/bin']);
    expect(pieces('Stops it; `harness-dashboard mcp` keeps running.')).toEqual([
      { text: 'Stops it; ', code: false },
      { text: 'harness-dashboard mcp', code: true },
      { text: ' keeps running.', code: false },
    ]);
  });

  it('puts back exactly the text it was given, minus the backticks', () => {
    for (const text of SIDE_EFFECTS) {
      expect(pieces(text).map((piece) => piece.text).join('')).toBe(text.replaceAll('`', ''));
    }
  });
});
