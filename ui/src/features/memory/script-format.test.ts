import { describe, expect, it } from 'vitest';

import fixture from './hidden-chars.fixture.json';
import { authorFacts, authorText, revealHidden, scopeText, shellQuote, tildePath } from './script-format';

describe('revealHidden', () => {
  it('leaves ordinary code, tabs, newlines and carriage returns alone', () => {
    const body = 'echo "héllo"\r\n\tls\n';

    expect(revealHidden(body)).toEqual({ segments: [{ text: body, hidden: false }], count: 0 });
  });

  it('escapes control, zero-width, bidi and byte order characters in place', () => {
    const { segments, count } = revealHidden('a\u001Bb\u200Fc\u2066d\uFEFF');

    expect(count).toBe(4);
    expect(segments.map((part) => part.text)).toEqual(['a', '⟨U+001B⟩', 'b', '⟨U+200F⟩', 'c', '⟨U+2066⟩', 'd', '⟨U+FEFF⟩']);
    expect(segments.filter((part) => part.hidden)).toHaveLength(4);
  });

  it('handles a body that starts with a hidden character', () => {
    expect(revealHidden('\u202Ex').segments).toEqual([
      { text: '⟨U+202E⟩', hidden: true },
      { text: 'x', hidden: false },
    ]);
  });
});

describe('revealHidden against hiding code', () => {
  const marks = (body: string) => revealHidden(body).segments.filter((part) => part.hidden).map((part) => part.text);

  it('flags a lone carriage return, which a terminal shows as the start of a new line', () => {
    expect(marks('x = 1  # note\rprint("CR-HIDDEN")')).toEqual(['⟨U+000D⟩']);
    expect(marks('a\r')).toEqual(['⟨U+000D⟩']);
    expect(marks('a\r\nb\r\n')).toEqual([]);
    expect(revealHidden('x\rn').segments[1]).toMatchObject({ lineBreak: true });
  });

  it('flags variation selectors that change nothing, and those from the supplement', () => {
    expect(marks('const ok\uFE00 = "EVIL"')).toEqual(['⟨U+FE00⟩']);
    expect(marks('a\uFE0Db')).toEqual(['⟨U+FE0D⟩']);
    expect(marks('a\u{E0100}b')).toEqual(['⟨U+E0100⟩']);
  });

  it('leaves the selectors and the joiner of an emoji alone', () => {
    expect(marks('\u2764\uFE0F ok')).toEqual([]);
    expect(marks('\u00A9\uFE0E')).toEqual([]);
    expect(marks('\u{1F468}\u200D\u{1F469}\u200D\u{1F467}')).toEqual([]);
    expect(marks('\u2764\uFE0F\u200D\u{1F525}')).toEqual([]);
    expect(marks('1\uFE0F\u20E3 #\uFE0F\u20E3 *\uFE0F\u20E3')).toEqual([]);
    expect(marks('\u2B50\u3030\uFE0F\u3297\uFE0F')).toEqual([]);
  });

  it('flags the same characters anywhere else', () => {
    expect(marks('a\u200Db')).toEqual(['⟨U+200D⟩']);
    expect(marks('a\uFE0Fb')).toEqual(['⟨U+FE0F⟩']);
    expect(marks('a\uFE0Eb')).toEqual(['⟨U+FE0E⟩']);
    expect(marks('\u{1F600}\u200D\u200D')).toEqual(['⟨U+200D⟩']);
    expect(marks('x1\uFE0F')).toEqual([]);
  });

  it('counts an emoji outside the basic plane once and keeps it whole', () => {
    expect(revealHidden('\u{1F600}\u202E')).toEqual({
      segments: [
        { text: '\u{1F600}', hidden: false },
        { text: '⟨U+202E⟩', hidden: true },
      ],
      count: 1,
    });
  });
});

/** The code point indexes revealHidden marks, to compare with the list the Memory server's own rule gives. */
function hiddenIndexes(body: string): number[] {
  const indexes: number[] = [];
  let position = 0;

  for (const part of revealHidden(body).segments) {
    if (part.hidden) {
      indexes.push(position);
      position += 1;
    } else {
      position += Array.from(part.text).length;
    }
  }

  return indexes;
}

describe('revealHidden against the shared fixture with the Memory server', () => {
  it.each(fixture.cases)('$name', ({ text, hidden }) => {
    expect(hiddenIndexes(text)).toEqual(hidden);
  });
});

describe('scopeText', () => {
  it('names the repository, falls back for a missing one, and says all projects for a global script', () => {
    expect(scopeText({ scope: 'repo', repo: 'github.com/o/r' })).toBe('github.com/o/r');
    expect(scopeText({ scope: 'repo', repo: null })).toBe('A repository');
    expect(scopeText({ scope: 'repo', repo: ' ' })).toBe('A repository');
    expect(scopeText({ scope: 'global', repo: null })).toBe('Global');
  });
});

describe('authorText', () => {
  it('adds the project root when it is known', () => {
    expect(authorText({ author: { source: 'claude-code', root: '/work/app' } })).toBe('claude-code · /work/app');
    expect(authorText({ author: { source: 'opencode', root: null } })).toBe('opencode');
  });
});

describe('shellQuote', () => {
  it('leaves plain arguments alone and quotes everything else the way a shell needs', () => {
    expect(shellQuote('--env=staging')).toBe('--env=staging');
    expect(shellQuote('./out/file.txt')).toBe('./out/file.txt');
    expect(shellQuote('two words')).toBe("'two words'");
    expect(shellQuote('')).toBe("''");
    expect(shellQuote("it's $HOME")).toBe("'it'\\''s $HOME'");
  });
});

describe('tildePath', () => {
  it('writes the home folder as a tilde on Linux, macOS and Windows, and leaves other folders alone', () => {
    expect(tildePath('/home/frogius/Projekty/harness-useful-logo')).toBe('~/Projekty/harness-useful-logo');
    expect(tildePath('/Users/mia/work/app')).toBe('~/work/app');
    expect(tildePath('C:\\Users\\mia\\work\\app')).toBe('~\\work\\app');
    expect(tildePath('/home/frogius')).toBe('~');
    expect(tildePath('/work/app')).toBe('/work/app');
    expect(tildePath('/homework/app')).toBe('/homework/app');
  });
});

describe('authorFacts', () => {
  it('splits who wrote it from the folder, which is cut at its start and keeps the whole path for the tooltip', () => {
    expect(authorFacts({ author: { source: 'omp-coding-agent', root: '/home/frogius/Projekty/harness-useful-logo' } })).toEqual([
      { label: 'Written by', value: 'omp-coding-agent' },
      { label: 'Folder', value: '~/Projekty/harness-useful-logo', title: '/home/frogius/Projekty/harness-useful-logo', cutStart: true },
    ]);
  });

  it('has no folder row when the folder is not known', () => {
    expect(authorFacts({ author: { source: 'opencode', root: null } })).toEqual([{ label: 'Written by', value: 'opencode' }]);
  });
});
