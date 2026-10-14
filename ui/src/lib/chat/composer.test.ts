import { describe, expect, it } from 'vitest';

import { bashCommand, messageAction, sendIntent } from './composer';

function key(init: Partial<Parameters<typeof sendIntent>[0]> = {}) {
  return { key: 'Enter', shiftKey: false, ctrlKey: false, metaKey: false, altKey: false, ...init };
}

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
