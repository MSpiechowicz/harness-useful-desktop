import type { DiffLine } from '$lib/ipc/bindings/DiffLine';
import type { FileDiff } from '$lib/ipc/bindings/FileDiff';

const ctx = (oldNo: number, newNo: number, text: string): DiffLine => ({ kind: 'context', oldNo, newNo, text });
const add = (newNo: number, text: string): DiffLine => ({ kind: 'add', oldNo: null, newNo, text });
const remove = (oldNo: number, text: string): DiffLine => ({ kind: 'remove', oldNo, newNo: null, text });

/**
 * A hand-written modified file with fixed numbers, so view tests do not move when the generated
 * samples are refreshed: two hunks, a replaced line, a missing final newline.
 */
export function modifiedFile(path = 'src/lib.txt'): FileDiff {
  return {
    path,
    oldPath: null,
    status: 'modified',
    staged: false,
    isBinary: false,
    truncated: false,
    additions: 12,
    deletions: 2,
    hunks: [
      {
        header: '@@ -1,6 +1,10 @@',
        oldStart: 1,
        oldLines: 6,
        newStart: 1,
        newLines: 10,
        lines: [
          ctx(1, 1, '//! Line-oriented record parser.'),
          ctx(2, 2, ''),
          add(3, 'mod error;'),
          add(4, ''),
          add(5, 'pub use error::Error;'),
          add(6, ''),
          remove(3, 'pub fn parse(input: &str) -> Result<Vec<Record>, String> {'),
          add(7, 'pub fn parse(input: &str) -> Result<Vec<Record>, Error> {'),
          ctx(4, 8, '    let mut records = Vec::new();'),
          ctx(5, 9, '    for line in input.lines() {'),
          ctx(6, 10, '        records.push(Record::from_line(line)?);'),
        ],
      },
      {
        header: '@@ -20,4 +24,12 @@ impl Record {',
        oldStart: 20,
        oldLines: 4,
        newStart: 24,
        newLines: 12,
        lines: [
          ctx(20, 24, '    pub fn is_empty(&self) -> bool {'),
          ctx(21, 25, '        self.fields.is_empty()'),
          ctx(22, 26, '    }'),
          remove(23, '}'),
          { kind: 'noNewline', oldNo: null, newNo: null, text: '' },
          add(27, '}'),
          add(28, ''),
          add(29, '#[cfg(test)]'),
        ],
      },
    ],
  };
}

export function binaryFile(path = 'assets/logo.png'): FileDiff {
  return { path, oldPath: null, status: 'modified', staged: false, isBinary: true, hunks: [], truncated: false, additions: 0, deletions: 0 };
}
