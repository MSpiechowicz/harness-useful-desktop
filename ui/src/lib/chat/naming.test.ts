import { afterEach, describe, expect, it } from 'vitest';

import { i18n } from '$lib/i18n/i18n.svelte';

import { collapse, crop, generatedName, generatedNameCount, labelFromPrompt, namingPrompt, parseLabel } from './naming';

describe('generated names', () => {
  afterEach(() => i18n.load('en'));

  it('are an adjective and a creature, and the numbering only shows once every pair is used', () => {
    expect(generatedName(1)).toBe('Swift Godzilla');
    expect(generatedName(2)).toBe('Swift Kraken');
    expect(generatedName(51)).toBe('Quiet Godzilla');
    const total = generatedNameCount();
    expect(generatedName(total)).not.toMatch(/\d$/);
    expect(generatedName(total + 1)).toBe('Swift Godzilla 2');
  });

  it('all differ from each other within one round', () => {
    const names = new Set(Array.from({ length: generatedNameCount() }, (_, index) => generatedName(index + 1)));

    expect(names.size).toBe(generatedNameCount());
  });

  it('use the adjective form that agrees with the creature in languages that have genders', async () => {
    await i18n.load('de');
    // Hydra is feminine, Godzilla masculine.
    expect(generatedName(1)).toBe('Flinker Godzilla');
    expect(generatedName(6)).toBe('Flinke Hydra');

    // The second adjective starts where the creatures run out: the first number whose creature is Godzilla again.
    const second = Array.from({ length: 200 }, (_, index) => index + 2).find((number) => generatedName(number).endsWith('Godzilla'))!;
    expect(generatedName(second)).toBe('Stiller Godzilla');
    expect(generatedName(second + 5)).toBe('Stille Hydra');

    await i18n.load('fr');
    const frSecond = Array.from({ length: 200 }, (_, index) => index + 2).find((number) => generatedName(number).startsWith('Godzilla'))!;
    expect(generatedName(frSecond + 5)).toBe('Hydre Discrète');
  });
});

describe('the number of generated names', () => {
  it('is every adjective with every creature, so a random pick stays inside the range', () => {
    expect(generatedNameCount()).toBe(36 * 50);
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
