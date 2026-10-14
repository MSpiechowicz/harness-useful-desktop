import { describe, expect, it } from 'vitest';

import { collapse, crop, generatedName, labelFromPrompt, namingPrompt, parseLabel } from './naming';

describe('generated names', () => {
  it('are creatures, not numbers, and the numbering only shows once the list is used up', () => {
    expect(generatedName(1)).toBe('Godzilla');
    expect(generatedName(2)).toBe('Kraken');
    expect(generatedName(30)).toBe('Behemoth');
    expect(generatedName(31)).toBe('Godzilla 2');
  });
});

describe('crop', () => {
  it('cuts at a word boundary and says so', () => {
    expect(crop('short', 40)).toBe('short');
    expect(crop('one two three four five six', 15)).toBe('one two three…');
  });
});

describe('label from the first message', () => {
  it('names the chat after its opening words and keeps more of the message as the description', () => {
    const label = labelFromPrompt('Fix the flaky login test.\n\nIt fails on CI only, about one run in five, with a timeout in the redirect step.');

    expect(label?.name).toBe('Fix the flaky login test. It fails on…');
    expect(label?.description).toMatch(/^Fix the flaky login test\. It fails on CI only/);
  });

  it('has no description when the title already says it all', () => {
    expect(labelFromPrompt('Explain this repo')).toEqual({ name: 'Explain this repo', description: '' });
  });

  it('ignores code blocks, markdown marks, slash commands and bash lines', () => {
    expect(collapse('# Title\n```ts\nconst a = 1;\n```\n**bold** text')).toBe('Title bold text');
    expect(labelFromPrompt('/compact')).toBeNull();
    expect(labelFromPrompt('!ls')).toBeNull();
    expect(labelFromPrompt('   ')).toBeNull();
  });
});

describe('the naming helper', () => {
  it('is asked with fixed instructions and no more than a short excerpt of the message', () => {
    const prompt = namingPrompt('word '.repeat(1000));

    expect(prompt.length).toBeLessThan(900);
    expect(prompt).toContain('JSON only');
  });

  it('parses the label out of fences and chatter, clamps it, and rejects anything else', () => {
    expect(parseLabel('Sure!\n```json\n{"name": "Login test fix", "description": "Debug the flaky login test."}\n```')).toEqual({ name: 'Login test fix', description: 'Debug the flaky login test.' });
    expect(parseLabel(`{"name": "${'x'.repeat(100)}"}`)?.name.length).toBeLessThanOrEqual(40);
    expect(parseLabel('{"name": ""}')).toBeNull();
    expect(parseLabel('no json here')).toBeNull();
    expect(parseLabel('{broken')).toBeNull();
  });
});
