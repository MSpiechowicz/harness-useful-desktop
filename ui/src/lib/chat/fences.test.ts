import { describe, expect, it } from 'vitest';

import { splitFences } from './fences';

describe('splitFences', () => {
  it('splits closed fences from the text around them and keeps the code indentation', () => {
    const source = '- Line 13: why?\n\n```jsx\n  <div className="a">\n```\n\n- Line 15: no';

    expect(splitFences(source)).toEqual([
      { type: 'text', text: '- Line 13: why?' },
      { type: 'code', lang: 'jsx', code: '  <div className="a">' },
      { type: 'text', text: '- Line 15: no' },
    ]);
  });

  it('keeps a fence that never closes as text, and a longer fence closes only on a fence as long', () => {
    expect(splitFences('```ts\nlet a')).toEqual([{ type: 'text', text: '```ts\nlet a' }]);
    expect(splitFences('````\n```\ninner\n```\n````')).toEqual([{ type: 'code', lang: '', code: '```\ninner\n```' }]);
  });
});
