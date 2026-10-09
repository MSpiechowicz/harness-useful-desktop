import { describe, expect, it } from 'vitest';

import { formatShortcut, matchesShortcut } from './shortcuts';

function key(init: KeyboardEventInit): KeyboardEvent {
  return new KeyboardEvent('keydown', init);
}

describe('matchesShortcut', () => {
  it('treats mod as Ctrl on non-Mac and Cmd on Mac', () => {
    expect(matchesShortcut(key({ key: 'k', ctrlKey: true }), 'mod+k', false)).toBe(true);
    expect(matchesShortcut(key({ key: 'k', metaKey: true }), 'mod+k', false)).toBe(false);
    expect(matchesShortcut(key({ key: 'k', metaKey: true }), 'mod+k', true)).toBe(true);
    expect(matchesShortcut(key({ key: 'k', ctrlKey: true }), 'mod+k', true)).toBe(false);
  });

  it('requires the exact modifier set', () => {
    expect(matchesShortcut(key({ key: 'k', ctrlKey: true, shiftKey: true }), 'mod+k', false)).toBe(false);
    expect(matchesShortcut(key({ key: 'k' }), 'mod+k', false)).toBe(false);
  });

  it('matches punctuation, digits and arrows', () => {
    expect(matchesShortcut(key({ key: ',', ctrlKey: true }), 'mod+,', false)).toBe(true);
    expect(matchesShortcut(key({ key: '3', ctrlKey: true }), 'mod+3', false)).toBe(true);
    expect(matchesShortcut(key({ key: 'ArrowLeft', altKey: true }), 'alt+arrowleft', false)).toBe(true);
  });
});

describe('formatShortcut', () => {
  it('renders platform-appropriate key caps', () => {
    expect(formatShortcut('mod+k', false)).toEqual(['Ctrl', 'K']);
    expect(formatShortcut('mod+shift+p', true)).toEqual(['⌘', '⇧', 'P']);
    expect(formatShortcut('alt+arrowleft', false)).toEqual(['Alt', '←']);
  });
});
