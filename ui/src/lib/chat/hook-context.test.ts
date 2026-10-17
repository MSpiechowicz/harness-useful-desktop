import { describe, expect, it } from 'vitest';

import { splitHookContext, splitHookFollowUp, splitHooks } from './hook-context';

const block = (...elements: string[]) => `<harness-hook-context>\n${elements.join('\n')}\n</harness-hook-context>`;
const element = (name: string, text: string) => `<hook name="${name}">${text}</hook>`;

describe('splitHookContext', () => {
  it('leaves a message without a block alone', () => {
    expect(splitHookContext('fix the bug')).toEqual({ hooks: [], text: 'fix the bug' });
    expect(splitHookContext('')).toEqual({ hooks: [], text: '' });
  });

  it('splits the hooks from the text after the blank line, in order', () => {
    const message = `${block(element('Search first', 'Look at the notes.'), element('Check limits', 'Mind the budget.\nTwo lines.'))}\n\nfix the bug\n\nplease`;

    expect(splitHookContext(message)).toEqual({
      hooks: [
        { name: 'Search first', text: 'Look at the notes.' },
        { name: 'Check limits', text: 'Mind the budget.\nTwo lines.' },
      ],
      text: 'fix the bug\n\nplease',
    });
  });

  it('gives an empty text when the block is all there is', () => {
    expect(splitHookContext(block(element('A', 'x'))).text).toBe('');
    expect(splitHookContext(block(element('A', 'x'))).hooks).toHaveLength(1);
  });

  it('unescapes names, and only the markers in texts', () => {
    const message = `${block(element('Fix &lt;a&gt; &amp; &quot;b&quot;', 'Never write &lt;/hook> or &lt;harness-hook-context>, and keep &lt;b and &amp; as they are'))}\n\nhi`;

    expect(splitHookContext(message).hooks).toEqual([
      { name: 'Fix <a> & "b"', text: 'Never write </hook> or <harness-hook-context>, and keep &lt;b and &amp; as they are' },
    ]);
  });

  it.each([
    ['text before the block', ` ${block(element('A', 'x'))}\n\nhi`],
    ['no blank line after the block', `${block(element('A', 'x'))}\nhi`],
    ['glued text', `${block(element('A', 'x'))}hi`],
    ['an empty block', '<harness-hook-context>\n</harness-hook-context>\n\nhi'],
    ['an unclosed block', `<harness-hook-context>\n${element('A', 'x')}\n\nhi`],
    ['two elements on one line', `<harness-hook-context>\n${element('A', 'x')}${element('B', 'y')}\n</harness-hook-context>\n\nhi`],
    ['a stray line between elements', `<harness-hook-context>\n${element('A', 'x')}\nnote\n${element('B', 'y')}\n</harness-hook-context>\n\nhi`],
    ['a name that never ends', '<harness-hook-context>\n<hook name="A>x</hook>\n</harness-hook-context>\n\nhi'],
  ])('keeps %s as plain text', (_label, message) => {
    expect(splitHookContext(message)).toEqual({ hooks: [], text: message });
  });
});

describe('splitHookFollowUp', () => {
  const followUp = (name: string, text: string) => `<harness-hook-followup name="${name}">${text}</harness-hook-followup>`;

  it('reads a message that is all follow-up', () => {
    expect(splitHookFollowUp(followUp('Save what you learned', 'Save notes.\nBriefly.'))).toEqual([{ name: 'Save what you learned', text: 'Save notes.\nBriefly.' }]);
  });

  it('reads several elements joined by a blank line', () => {
    expect(splitHookFollowUp(`${followUp('A', 'x')}\n\n${followUp('B &amp; C', 'y')}`)).toEqual([
      { name: 'A', text: 'x' },
      { name: 'B & C', text: 'y' },
    ]);
  });

  it.each([
    ['text around it', `hi ${followUp('A', 'x')}`],
    ['text after it', `${followUp('A', 'x')} thanks`],
    ['a single line break between elements', `${followUp('A', 'x')}\n${followUp('B', 'y')}`],
    ['no closing tag', '<harness-hook-followup name="A">x'],
    ['the user writing about it', 'What does <harness-hook-followup name="A">x</harness-hook-followup> mean?'],
    ['an empty message', ''],
  ])('does not read %s', (_label, message) => {
    expect(splitHookFollowUp(message)).toBeNull();
  });
});

describe('splitHooks', () => {
  it('tells a hook follow-up from a context block from the user\'s own words', () => {
    expect(splitHooks('hello')).toEqual({ text: 'hello', context: [], followUp: [] });
    expect(splitHooks(`${block(element('A', 'x'))}\n\nhello`)).toEqual({ text: 'hello', context: [{ name: 'A', text: 'x' }], followUp: [] });
    expect(splitHooks('<harness-hook-followup name="A">x</harness-hook-followup>')).toEqual({ text: '', context: [], followUp: [{ name: 'A', text: 'x' }] });
  });
});
