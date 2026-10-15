import { describe, expect, it } from 'vitest';

import { bashCommand, escapeStep, latestWithdrawable, messageAction, queuedRows, sendIntent, STOP_WINDOW_MS } from './composer';

function key(init: Partial<Parameters<typeof sendIntent>[0]> = {}) {
  return { key: 'Enter', shiftKey: false, ctrlKey: false, metaKey: false, altKey: false, ...init };
}

describe('the queue above the composer', () => {
  it('keeps steering the agent already took out of reach, and takes back the newest steering first', () => {
    const rows = queuedRows({ steering: ['live', 'waiting'], followUp: ['later'], liveSteered: 1 });

    expect(rows.map((row) => [row.message, row.queue, row.withdrawable])).toEqual([
      ['live', 'steer', false],
      ['waiting', 'steer', true],
      ['later', 'followUp', true],
    ]);
    expect(latestWithdrawable(rows)?.message).toBe('waiting');
  });

  it('falls back to the newest follow-up, and to nothing when only live steering is left', () => {
    expect(latestWithdrawable(queuedRows({ steering: ['live'], followUp: ['a', 'b'], liveSteered: 1 }))?.message).toBe('b');
    expect(latestWithdrawable(queuedRows({ steering: ['live'], followUp: [], liveSteered: 1 }))).toBeNull();
  });
});

describe('Enter in the composer', () => {
  it('sends when the agent is idle, whatever the modifiers', () => {
    expect(sendIntent(key(), false)).toBe('send');
    expect(sendIntent(key({ ctrlKey: true }), false)).toBe('send');
    expect(sendIntent(key({ altKey: true }), false)).toBe('send');
  });

  it('leaves Shift+Enter as a new line', () => {
    expect(sendIntent(key({ shiftKey: true }), false)).toBeNull();
    expect(sendIntent(key({ shiftKey: true }), true)).toBeNull();
  });

  it('steers on Enter and Alt+Enter while streaming, follows up on Ctrl/Cmd+Enter', () => {
    expect(sendIntent(key(), true)).toBe('steer');
    expect(sendIntent(key({ altKey: true }), true)).toBe('steer');
    expect(sendIntent(key({ ctrlKey: true }), true)).toBe('followUp');
    expect(sendIntent(key({ metaKey: true }), true)).toBe('followUp');
  });

  it('does the other mode on Ctrl/Cmd+Enter when Queue is the default', () => {
    expect(sendIntent(key(), true, 'followUp')).toBe('followUp');
    expect(sendIntent(key({ ctrlKey: true }), true, 'followUp')).toBe('steer');
    expect(sendIntent(key(), false, 'followUp')).toBe('send');
  });

  it('ignores other keys and IME composition', () => {
    expect(sendIntent(key({ key: 'a' }), false)).toBeNull();
    expect(sendIntent(key({ isComposing: true }), false)).toBeNull();
  });
});

describe('messageAction', () => {
  it('maps each mode to the action that delivers it', () => {
    expect(messageAction('hi', 'send')).toEqual({ type: 'prompt', message: 'hi', images: [], streamingBehavior: null });
    expect(messageAction('hi', 'steer')).toEqual({ type: 'steer', message: 'hi', images: [] });
    expect(messageAction('hi', 'followUp')).toEqual({ type: 'followUp', message: 'hi', images: [] });
  });

  it('carries the images', () => {
    const images = [{ data: 'aGk=', mimeType: 'image/png' }];

    expect(messageAction('hi', 'send', images)).toMatchObject({ type: 'prompt', images });
    expect(messageAction('hi', 'steer', images)).toMatchObject({ type: 'steer', images });
  });
});

describe('bashCommand', () => {
  it('is the text after the bang', () => {
    expect(bashCommand('!git status')).toBe('git status');
    expect(bashCommand('!  ls -la ')).toBe('ls -la');
  });

  it('is null for ordinary text and an empty command', () => {
    expect(bashCommand('git status')).toBeNull();
    expect(bashCommand('!')).toBeNull();
    expect(bashCommand('! ')).toBeNull();
  });
});

describe('double Escape', () => {
  it('arms on the first press and stops on a second one inside the window', () => {
    expect(escapeStep(null, 1000)).toBe('arm');
    expect(escapeStep(1000, 1000 + STOP_WINDOW_MS)).toBe('stop');
  });

  it('starts over when the second press comes too late', () => {
    expect(escapeStep(1000, 1000 + STOP_WINDOW_MS + 1)).toBe('arm');
  });
});
