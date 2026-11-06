import { describe, expect, it } from 'vitest';

import { questionTitle } from './question-title';

describe('questionTitle', () => {
  it('splits the step off an OMP title', () => {
    expect(questionTitle('How much latitude? (2/3)')).toEqual({ text: 'How much latitude?', step: { number: 2, total: 3 } });
  });

  it.each(['Ratio (5/3)?', 'Ratio (5/3)', 'Ratio (0/3)', 'Only (1/1)', 'Plain title'])(
    'keeps %s whole when it holds no valid step',
    (title) => {
      expect(questionTitle(title)).toEqual({ text: title, step: null });
    },
  );
});
