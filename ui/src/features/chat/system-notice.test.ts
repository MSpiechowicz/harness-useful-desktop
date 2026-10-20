import { describe, expect, it } from 'vitest';

import { unwrapNotice } from './system-notice';

describe('unwrapNotice', () => {
  it('titles a wrapped notice by its tag, keeps its headline and puts the rest apart as details', () => {
    expect(unwrapNotice('<system-notice>\nBackground job bg_2 has completed. Resume your work using the result below.\nerror[E0308]: mismatched types\n  --> a.rs:1\n</system-notice>')).toEqual({
      title: 'System notice',
      body: 'Background job bg_2 has completed.',
      details: 'error[E0308]: mismatched types\n  --> a.rs:1',
    });
  });

  it('has no details when the notice is one line', () => {
    expect(unwrapNotice('<system-notice>Job done.</system-notice>')).toEqual({ title: 'System notice', body: 'Job done.', details: null });
  });

  it('reads a notice with a very long run of spaces at once', () => {
    const started = performance.now();
    const padded = `<system-notice>${' '.repeat(200_000)}done${' '.repeat(200_000)}`;

    expect(unwrapNotice(padded).title).toBeNull();
    expect(unwrapNotice(`${padded}</system-notice>`)).toEqual({ title: 'System notice', body: 'done', details: null });
    expect(performance.now() - started).toBeLessThan(500);
  });

  it('leaves text that is not one wrapped block as it is', () => {
    for (const text of ['plain text', '<a>one</a> and <b>two</b>', 'before <a>one</a>']) {
      expect(unwrapNotice(text)).toEqual({ title: null, body: text, details: null });
    }
  });
});
