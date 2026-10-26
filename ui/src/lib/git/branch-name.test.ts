import { describe, expect, it } from 'vitest';

import { isBranchName, slugBranchName } from './branch-name';

describe('slugBranchName', () => {
  it('turns words into a dashed name and keeps folders', () => {
    expect(slugBranchName('  feat/rebase picker  ')).toBe('feat/rebase-picker');
  });

  it('keeps letters of any language and drops what git forbids', () => {
    expect(slugBranchName('nowa gałąź?*')).toBe('nowa-gałąź');
    expect(slugBranchName('a~b^c:d[e\\f')).toBe('abcdef');
  });

  it('tidies separators and dots', () => {
    expect(slugBranchName('//a//b//')).toBe('a/b');
    expect(slugBranchName('a..b')).toBe('a.b');
    expect(slugBranchName('.hidden/x.lock/y.')).toBe('hidden/x/y');
    expect(slugBranchName('--flag')).toBe('flag');
  });

  it('is empty when nothing usable was typed', () => {
    expect(slugBranchName('  ?? ')).toBe('');
  });

  it('always yields a name git accepts, unless it is a reserved word', () => {
    for (const typed of ['feat/new thing', 'fix: the bug (#12)', '.. x', 'release/1.0.lock']) {
      expect(isBranchName(slugBranchName(typed)), typed).toBe(true);
    }
  });
});

describe('isBranchName', () => {
  it.each(['main', 'feat/branch-sync', 'release/1.2.3', 'żółw', 'a@b'])('accepts %s', (name) => {
    expect(isBranchName(name)).toBe(true);
  });

  it.each(['', '-x', 'HEAD', '@', 'a..b', 'a b', 'a//b', '/a', 'a/', 'a.', 'a/.b', 'a.lock', 'a/b.lock/c', 'a@{b', 'a~b', 'a^b', 'a:b', 'a?b', 'a*b', 'a[b', 'a\\b', 'a\u0007b'])(
    'refuses %j',
    (name) => {
      expect(isBranchName(name)).toBe(false);
    },
  );

  it('refuses a name over 255 bytes', () => {
    expect(isBranchName('a'.repeat(255))).toBe(true);
    expect(isBranchName('a'.repeat(256))).toBe(false);
  });
});
