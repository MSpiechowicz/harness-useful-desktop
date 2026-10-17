import { describe, expect, it } from 'vitest';

import { en } from './locales/en';

type Area = Record<string, string>;
type Modules = Record<string, Record<string, Area>>;

const LANGUAGES = ['de', 'es', 'fr', 'pl'] as const;
const EXTRA_FORM = /\.(zero|two|few|many)$/;

// Every area file of every language, by path: ./locales/<lang>/<area>.ts
const modules = import.meta.glob('./locales/*/*.ts', { eager: true }) as Modules;

function areasOf(lang: string): Record<string, Area> {
  const areas: Record<string, Area> = {};

  for (const [path, module] of Object.entries(modules)) {
    const match = /^\.\/locales\/([a-z]+)\/([a-z]+)\.ts$/.exec(path);

    if (match && match[1] === lang && match[2] !== 'index') {
      areas[match[2]] = module[match[2]];
    }
  }

  return areas;
}

function placeholders(message: string): string[] {
  return [...new Set([...message.matchAll(/\{(\w+)\}/g)].map((match) => match[1]))].sort();
}

/** The English message a translated key is compared with: the plural extras compare with `.other`. */
function english(key: string): string | undefined {
  const messages = en as Area;

  return messages[key] ?? messages[key.replace(EXTRA_FORM, '.other')];
}

const pluralBases = Object.keys(en)
  .filter((key) => key.endsWith('.other'))
  .map((key) => key.slice(0, -'.other'.length));

describe('English catalog', () => {
  it('has the fifteen areas, each key under its own area and no key twice', () => {
    const areas = areasOf('en');

    expect(Object.keys(areas).sort()).toEqual(
      ['changes', 'chat', 'common', 'components', 'config', 'ecosystem', 'extensions', 'harness', 'memory', 'onboarding', 'sessions', 'settings', 'shell', 'sync', 'updates'],
    );

    let total = 0;
    for (const [area, messages] of Object.entries(areas)) {
      for (const key of Object.keys(messages)) {
        expect(key.startsWith(`${area}.`), key).toBe(true);
      }
      total += Object.keys(messages).length;
    }

    expect(Object.keys(en)).toHaveLength(total);
  });

  it('has no empty message', () => {
    for (const [key, message] of Object.entries(en)) {
      expect(message.trim(), key).not.toBe('');
    }
  });
});

describe.each(LANGUAGES)('%s catalog', (lang) => {
  const areas = areasOf(lang);
  const messages = Object.assign({}, ...Object.values(areas)) as Area;

  it('has the same areas as English', () => {
    expect(Object.keys(areas).sort()).toEqual(Object.keys(areasOf('en')).sort());
  });

  it('has exactly the English keys, apart from the extra plural forms', () => {
    const own = Object.keys(messages).filter((key) => !(EXTRA_FORM.test(key) && key.replace(EXTRA_FORM, '.other') in en));

    expect(own.sort()).toEqual(Object.keys(en).sort());
  });

  it('keeps every key under its area', () => {
    for (const [area, group] of Object.entries(areas)) {
      for (const key of Object.keys(group)) {
        expect(key.startsWith(`${area}.`), key).toBe(true);
      }
    }
  });

  it('keeps the placeholders of the English message', () => {
    for (const [key, message] of Object.entries(messages)) {
      expect(placeholders(message), key).toEqual(placeholders(english(key) ?? ''));
    }
  });

  it('has no empty message', () => {
    for (const [key, message] of Object.entries(messages)) {
      expect(message.trim(), key).not.toBe('');
    }
  });
});

describe('Polish plurals', () => {
  it('has the few and many forms of every plural', () => {
    const messages = Object.assign({}, ...Object.values(areasOf('pl'))) as Area;

    for (const base of pluralBases) {
      expect(messages[`${base}.few`], `${base}.few`).toBeTruthy();
      expect(messages[`${base}.many`], `${base}.many`).toBeTruthy();
    }
  });
});
