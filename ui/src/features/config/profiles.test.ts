import { describe, expect, it } from 'vitest';

import type { Notice } from '$lib/ipc/bindings/Notice';

import { splitProfileNotice } from './profiles';

const info = (message: string): Notice => ({ level: 'info', message });

describe('splitProfileNotice', () => {
  it('lifts the profile names and the hint out of the notice', () => {
    const other = info('The project layer is not trusted.');
    const result = splitProfileNotice([other, info('Profiles: smoke, us-issue8-a6lz3fhc. Start OMP with --profile <name> to use one.')]);

    expect(result.names).toEqual(['smoke', 'us-issue8-a6lz3fhc']);
    expect(result.hint).toBe('Start OMP with --profile <name> to use one.');
    expect(result.rest).toEqual([other]);
  });

  it('keeps dots inside a name', () => {
    expect(splitProfileNotice([info('Profiles: v1.2, dev. Start OMP with --profile <name> to use one.')]).names).toEqual(['v1.2', 'dev']);
  });

  it('accepts a list without a hint', () => {
    expect(splitProfileNotice([info('Profiles: a, b.')])).toEqual({ names: ['a', 'b'], hint: null, rest: [] });
  });

  it('leaves anything it does not recognise as a notice', () => {
    const notices = [info('Active profile: work. Other profiles: a, b.'), { level: 'warning', message: 'Profiles: a. Start OMP.' } as Notice, info('Profiles: .')];

    expect(splitProfileNotice(notices)).toEqual({ names: [], hint: null, rest: notices });
  });
});
