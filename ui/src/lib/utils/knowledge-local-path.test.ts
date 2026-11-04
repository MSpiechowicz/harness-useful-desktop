import { describe, expect, it } from 'vitest';

import { withoutFolderPaths } from './knowledge-local-path';

const key = (path: string) => ({ repo: `path:${path}` });

describe('withoutFolderPaths', () => {
  it('cuts a whole path to its last folder wherever a sentence puts it', () => {
    const links = [key('/home/ada/private/notes')];

    expect(withoutFolderPaths('/home/ada/private/notes has no git remote', links)).toBe('notes has no git remote');
    expect(withoutFolderPaths('cannot link "/home/ada/private/notes", it stays here', links)).toBe('cannot link "notes", it stays here');
    expect(withoutFolderPaths("(path:/home/ada/private/notes) is a folder.", links)).toBe('(path:notes) is a folder.');
    expect(withoutFolderPaths('refused /home/ada/private/notes.', links)).toBe('refused notes.');
    expect(withoutFolderPaths('refused C:\\work\\notes now', [key('C:\\work\\notes')])).toBe('refused notes now');
  });

  it('replaces the longer path first, so a path that starts another one leaves no partial path', () => {
    const links = [key('/home/ada/work'), key('/home/ada/work/widget')];

    expect(withoutFolderPaths('/home/ada/work/widget and /home/ada/work are folders', links)).toBe('widget and work are folders');
    expect(withoutFolderPaths('/home/ada/work/widget and /home/ada/work are folders', [...links].reverse())).toBe('widget and work are folders');
  });

  it('leaves text that only looks like a path, or has it inside a longer name, as it is, and shortens a path inside the folder', () => {
    const links = [key('/home/ada/work')];

    expect(withoutFolderPaths('see /home/ada/work/widget/src', links)).toBe('see work/widget/src');
    expect(withoutFolderPaths('see /home/ada/workshop and /home/ada/work.md', links)).toBe('see /home/ada/workshop and /home/ada/work.md');
    expect(withoutFolderPaths('see /srv/home/ada/work', links)).toBe('see /srv/home/ada/work');
  });

  it('leaves the text alone when no link names a folder', () => {
    expect(withoutFolderPaths('/home/ada/work', [{ repo: 'git:github.com/acme/widget' }])).toBe('/home/ada/work');
  });
});

describe('a path that goes on past the folder', () => {
  it('replaces only the folder part of a file path inside it, and a trailing separator', () => {
    const links = [{ repo: 'path:/home/u/repo' }];

    expect(withoutFolderPaths('could not read /home/u/repo/docs/x.md', links)).toBe('could not read repo/docs/x.md');
    expect(withoutFolderPaths('in /home/u/repo/ now', links)).toBe('in repo/ now');
    expect(withoutFolderPaths('C:\\Users\\u\\repo\\a.md', [{ repo: 'path:C:\\Users\\u\\repo' }])).toBe('repo\\a.md');
  });

  it('still leaves a longer folder name alone', () => {
    expect(withoutFolderPaths('/home/u/repository/x', [{ repo: 'path:/home/u/repo' }])).toBe('/home/u/repository/x');
  });
});
