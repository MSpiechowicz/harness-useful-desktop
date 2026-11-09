import { describe, expect, it } from 'vitest';

import { isBashDraft } from './composer';
import { applyFileMention, fileMentionQuery } from './file-mention';

/** The mention for a draft written with `|` where the caret is. */
function at(marked: string) {
  const caret = marked.indexOf('|');
  const draft = marked.replace('|', '');

  return fileMentionQuery(draft, caret);
}

describe('fileMentionQuery', () => {
  it.each([
    ['@|', { start: 0, end: 1, query: '' }],
    ['@src/li|', { start: 0, end: 7, query: 'src/li' }],
    ['look at @src/li|', { start: 8, end: 15, query: 'src/li' }],
    ['look\n@src|', { start: 5, end: 9, query: 'src' }],
    ['see (@src|', { start: 5, end: 9, query: 'src' }],
    ['see [@src|', { start: 5, end: 9, query: 'src' }],
    ['see {@src|', { start: 5, end: 9, query: 'src' }],
    ['say "@src|', { start: 5, end: 9, query: 'src' }],
    ["say '@src|", { start: 5, end: 9, query: 'src' }],
    ['@node_modules/@types/no|', { start: 0, end: 23, query: 'node_modules/@types/no' }],
    ['/review @src/x|', { start: 8, end: 14, query: 'src/x' }],
    ['(@sr|c)', { start: 1, end: 5, query: 'sr' }],
    ['(@sr|c) now', { start: 1, end: 5, query: 'sr' }],
    ['see @foo|, then', { start: 4, end: 8, query: 'foo' }],
    ['[@a|];', { start: 1, end: 3, query: 'a' }],
    ['{@a|}', { start: 1, end: 3, query: 'a' }],
    ['"@a|"', { start: 1, end: 3, query: 'a' }],
    ["'@a|'", { start: 1, end: 3, query: 'a' }],
    ['@a|;b', { start: 0, end: 2, query: 'a' }],
    ['x @a|`', { start: 2, end: 4, query: 'a' }],
  ])('opens for %s', (marked, expected) => {
    expect(at(marked)).toEqual(expected);
  });

  it.each([
    ['mail me at a@b|.com'],
    ['a@b|'],
    ['no mention here|'],
    ['@src |'],
    ['|@src'],
    ['`code @sr|'],
    ['a `b` and `c @sr|'],
    ['```\n@sr|'],
    ['~~~ts\ncode\n@sr|'],
    ['!ls @sr|'],
  ])('stays closed for %s', (marked) => {
    expect(at(marked)).toBeNull();
  });

  it.each([
    ['@src/ro|utes/[id]/page', { start: 0, end: 21, query: 'src/ro' }],
    ['@a/(app)/pa|ge', { start: 0, end: 13, query: 'a/(app)/pa' }],
    ['@a/(ap|p)/x', { start: 0, end: 10, query: 'a/(ap' }],
    ['go @a/|{x}/z now', { start: 3, end: 11, query: 'a/' }],
    ['(@a/ro|utes/[id]/page)', { start: 1, end: 20, query: 'a/ro' }],
    ['[@a/(g)/|b]', { start: 1, end: 9, query: 'a/(g)/' }],
  ])('ends a path that has brackets of its own only at the bracket that closes nothing in it, for %s', (marked, expected) => {
    expect(at(marked)).toEqual(expected);
  });

  it('opens again after a code span or a fence has closed', () => {
    expect(at('`a` @sr|')).toMatchObject({ query: 'sr' });
    expect(at('```\ncode\n```\n@sr|')).toMatchObject({ query: 'sr' });
  });

  it('takes the query up to the caret and replaces the whole token', () => {
    expect(at('fix @src/|lib.rs now')).toEqual({ start: 4, end: 15, query: 'src/' });
  });

  it('stays closed for a selection, even inside a mention', () => {
    expect(fileMentionQuery('@src/lib.rs', 2, 6)).toBeNull();
  });

  it('stays closed for a caret outside the draft', () => {
    expect(fileMentionQuery('@src', 9)).toBeNull();
  });
});

describe('applyFileMention', () => {
  it('keeps a file whose name starts with ! from turning the draft into a shell command', () => {
    const { text } = applyFileMention('@ec', { start: 0, end: 3, query: 'ec' }, { path: '!echo hi', isDir: false });

    expect(isBashDraft(text)).toBe(false);
    expect(text).toBe('./!echo hi ');
  });

  it('keeps a name starting with @ from reading as a mention, for a file and a folder', () => {
    const mention = { start: 0, end: 3, query: 'sc' };

    expect(applyFileMention('@sc', mention, { path: '@scope/x', isDir: false }).text).toBe('./@scope/x ');
    expect(applyFileMention('@sc', mention, { path: '@scope', isDir: true }).text).toBe('./@scope/ ');
  });

  it('replaces the whole path, brackets and all, when the caret is in the middle of it', () => {
    const draft = 'open @src/ro|utes/[id]/page now';
    const mention = fileMentionQuery(draft.replace('|', ''), draft.indexOf('|'));

    expect(applyFileMention(draft.replace('|', ''), mention!, { path: 'src/routes/[id]/page.ts', isDir: false }).text).toBe('open src/routes/[id]/page.ts now');
  });

  const FILE = { path: 'src/lib.rs', isDir: false };

  it('replaces the mention at the end of the draft with the path and a space', () => {
    const draft = 'read @src/li';

    expect(applyFileMention(draft, { start: 5, end: 12, query: 'src/li' }, FILE)).toEqual({ text: 'read src/lib.rs ', caret: 16 });
  });

  it('replaces the mention in the middle and keeps the rest', () => {
    const draft = 'read @src/li then fix it';

    expect(applyFileMention(draft, { start: 5, end: 12, query: 'src/li' }, FILE)).toEqual({ text: 'read src/lib.rs then fix it', caret: 15 });
  });

  it('adds no second space when whitespace already follows', () => {
    const next = applyFileMention('@sr\nnext', { start: 0, end: 3, query: 'sr' }, FILE);

    expect(next.text).toBe('src/lib.rs\nnext');
    expect(next.caret).toBe(10);
  });

  it('gives a folder a trailing slash', () => {
    expect(applyFileMention('@sr', { start: 0, end: 3, query: 'sr' }, { path: 'src', isDir: true })).toEqual({ text: 'src/ ', caret: 5 });
  });

  it.each([
    ['(@sr|c)', '(src/lib.rs)', 11],
    ['see @foo|, then', 'see src/lib.rs, then', 14],
    ['"@sr|"', '"src/lib.rs"', 11],
    ['@sr|c.rs now', 'src/lib.rs now', 10],
  ])('keeps the punctuation after the token when %s is picked', (marked, text, caret) => {
    const draft = marked.replace('|', '');
    const mention = fileMentionQuery(draft, marked.indexOf('|'));

    expect(mention).not.toBeNull();
    expect(applyFileMention(draft, mention!, FILE)).toEqual({ text, caret });
  });
});
