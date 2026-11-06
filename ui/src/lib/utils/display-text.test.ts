import { describe, expect, it } from 'vitest';

import { displayText, markedText, visibleText } from './display-text';

describe('text of another program as it is shown', () => {
  it('drops control, bidi and zero-width characters and trims', () => {
    expect(displayText('  a‮b​c\u0007d e️  ', 50)).toBe('abcde');
  });

  it('cuts to a number of characters, never in the middle of one', () => {
    expect(displayText('😀😀😀', 2)).toBe('😀😀');
    expect(displayText('abc def', 4)).toBe('abc');
  });
});

describe('visibleText', () => {
  it('drops hidden characters and keeps the lines', () => {
    expect(visibleText('Allow\u202E tool\u200B:\nline\ttwo\u0007')).toBe('Allow tool:\nline\ttwo');
  });
});

describe('line breaks in visibleText', () => {
  it('keeps a break another program writes in any form', () => {
    expect(visibleText('a\rb\u2028c')).toBe('a\nb\nc');
  });
});

describe('markedText', () => {
  it('names hidden characters and keeps everything else', () => {
    expect(markedText('rm\u202E -rf\nnext\r')).toBe('rm⟦U+202E⟧ -rf\nnext⟦U+000D⟧');
  });
});
