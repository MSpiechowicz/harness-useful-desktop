import { describe, expect, it } from 'vitest';

import { readStepStatus, saysNone, splitStepStatus } from './step-status';

const TABLE = '| Step | Status |\n| --- | --- |\n| Complexity | small, a one-line fix |\n| Code Graph | code_map `src/a.ts` |';

describe('splitStepStatus', () => {
  it('lifts the table out of the answer', () => {
    expect(splitStepStatus(`${TABLE}\n\nDone.`)).toEqual({ before: '', table: TABLE, after: 'Done.' });
  });

  it('keeps the text an agent wrote before the table', () => {
    expect(splitStepStatus(`Fixed.\n\n${TABLE}\n\nDone.`)).toEqual({ before: 'Fixed.', table: TABLE, after: 'Done.' });
  });

  it('leaves text without a status alone', () => {
    expect(splitStepStatus('Just an answer.')).toBeNull();
    expect(splitStepStatus('| Name | Value |\n| --- | --- |\n| Notes | one |')).toBeNull();
  });

  it('leaves a rollout table with the same headers in the answer', () => {
    expect(splitStepStatus('| Step | Status |\n| --- | --- |\n| Build | passed |\n| Deploy | failed |')).toBeNull();
  });

  it('leaves a status shown as an example in a code block', () => {
    expect(splitStepStatus(`\`\`\`md\n${TABLE}\n\`\`\``)).toBeNull();
  });

  it('finds the status after another table with the same headers', () => {
    const other = '| Step | Status |\n| --- | --- |\n| Build | passed |';

    expect(splitStepStatus(`${other}\n\n${TABLE}\n\nDone.`)).toEqual({ before: other, table: TABLE, after: 'Done.' });
  });

  it('does not close a code block on another kind of fence', () => {
    expect(splitStepStatus(`\`\`\`md\n~~~\n${TABLE}\n\`\`\``)).toBeNull();
  });

  it('accepts a table without the spaces around its pipes', () => {
    const table = '|Step|Status|\n|---|---|\n|Notes|none fit|';

    expect(splitStepStatus(table)?.table).toBe(table);
  });

  describe('while the text arrives', () => {
    it.each(['|', '| St', '| Step | Status', '| Step | Status |', '| Step | Status |\n', '| Step | Status |\n| --', '| Step | Status |\n| --- | --- |\n', '| Step | Status |\n| --- | --- |\n| Compl'])(
      'holds back %j instead of showing it as text',
      (text) => {
        expect(splitStepStatus(text, true)).toEqual({ before: '', table: '', after: '' });
      },
    );

    it('adds a row once its line is complete', () => {
      const head = '| Step | Status |\n| --- | --- |\n| Notes | none fit |';

      expect(splitStepStatus(`${head}\n| Scripts | no`, true)).toEqual({ before: '', table: head, after: '' });
      expect(splitStepStatus(`${head}\n`, true)).toEqual({ before: '', table: head, after: '' });
    });

    it('streams the answer after the table', () => {
      expect(splitStepStatus(`${TABLE}\n\nDo`, true)).toEqual({ before: '', table: TABLE, after: 'Do' });
    });

    it('lets another table go as soon as it cannot be the status', () => {
      expect(splitStepStatus('| Name', true)).toBeNull();
      expect(splitStepStatus('| Step | Owner |\n', true)).toBeNull();
      expect(splitStepStatus('| Step | Status |\n| --- | --- |\n| Build | passed |\n\nAfter', true)).toBeNull();
    });

    it('shows a table held back to the end as text when the reply stops there', () => {
      expect(splitStepStatus('| Step | Status |')).toBeNull();
    });
  });
});

describe('readStepStatus', () => {
  const table = (...rows: string[]) => ['| Step | Status |', '| --- | --- |', ...rows].join('\n');

  it.each([
    ['small — a one-line fix', 'easy', 'a one-line fix'],
    ['small, a one-line fix', 'easy', 'a one-line fix'],
    ['medium - two views', 'medium', 'two views'],
    ['full: a new card', 'hard', 'a new card'],
    ['full', 'hard', null],
  ])('reads %j as a level and its reason', (value, level, reason) => {
    expect(readStepStatus(table(`| Complexity | ${value} |`))).toMatchObject({ level, reason });
  });

  it('shows no badge for a word it does not know, and keeps what the agent wrote', () => {
    expect(readStepStatus(table('| Complexity | tricky, it needs care |'))).toMatchObject({ level: null, reason: 'tricky, it needs care' });
  });

  it('keeps the other rows in order, and drops a limit that is not close', () => {
    const { rows } = readStepStatus(table('| Limits | none close |', '| Branch | merged 3 commits |', '| Complexity | small |', '| Notes | none fit |'));

    expect(rows).toEqual([{ label: 'Branch', value: 'merged 3 commits' }, { label: 'Notes', value: 'none fit' }]);
  });

  it('keeps a limit that is close', () => {
    expect(readStepStatus(table('| Limits | Claude weekly window 87% used |')).rows).toEqual([{ label: 'Limits', value: 'Claude weekly window 87% used' }]);
  });

  it('tells a line that says nothing was used', () => {
    expect(saysNone('none fit')).toBe(true);
    expect(saysNone('nonexistent.ts')).toBe(false);
  });
});
