import { describe, expect, it } from 'vitest';

import { matchOptions } from './menu-search';

const OPTIONS = [
  { label: 'widget', hint: 'acme/widget' },
  { label: 'Alpha', hint: 'Remote only' },
  { label: 'Global' },
];

describe('matchOptions', () => {
  it('keeps every option while nothing is typed', () => {
    expect(matchOptions(OPTIONS, '  ')).toEqual(OPTIONS);
  });

  it('matches the label or the hint in any case', () => {
    expect(matchOptions(OPTIONS, 'GLO').map((option) => option.label)).toEqual(['Global']);
    expect(matchOptions(OPTIONS, 'acme/').map((option) => option.label)).toEqual(['widget']);
    expect(matchOptions(OPTIONS, 'remote').map((option) => option.label)).toEqual(['Alpha']);
    expect(matchOptions(OPTIONS, 'nothing')).toEqual([]);
  });
});
