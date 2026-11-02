import { describe, expect, it } from 'vitest';

import { displayText } from './display-text';

describe('text of another program as it is shown', () => {
  it('drops control, bidi and zero-width characters and trims', () => {
    expect(displayText('  a‮b​c\u0007d e️  ', 50)).toBe('abcde');
  });

  it('cuts to a number of characters, never in the middle of one', () => {
    expect(displayText('😀😀😀', 2)).toBe('😀😀');
    expect(displayText('abc def', 4)).toBe('abc');
  });
});
