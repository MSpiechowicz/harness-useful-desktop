import { en, type MessageKey } from './locales/en';
import type { Dict } from './locales/types';

export type Lang = 'en' | 'de' | 'es' | 'fr' | 'pl' | 'pt';

/** Each language named in itself, as a language picker lists them. */
export const LANGS: readonly { code: Lang; label: string }[] = [
  { code: 'en', label: 'English' },
  { code: 'de', label: 'Deutsch' },
  { code: 'es', label: 'Español' },
  { code: 'fr', label: 'Français' },
  { code: 'pl', label: 'Polski' },
  { code: 'pt', label: 'Português (Brasil)' },
];

// English is built in. The others load when chosen, each from its own chunk.
const LOADERS: Record<Exclude<Lang, 'en'>, () => Promise<Dict>> = {
  de: () => import('./locales/de').then((module) => module.de),
  es: () => import('./locales/es').then((module) => module.es),
  fr: () => import('./locales/fr').then((module) => module.fr),
  pl: () => import('./locales/pl').then((module) => module.pl),
  pt: () => import('./locales/pt').then((module) => module.pt),
};

const LOCALES: Record<Lang, string> = { en: 'en-US', de: 'de-DE', es: 'es-ES', fr: 'fr-FR', pl: 'pl-PL', pt: 'pt-BR' };

/** The saved language code as a supported language; anything unknown is English. */
export function toLang(code: string | null | undefined): Lang {
  return LANGS.find((lang) => lang.code === code)?.code ?? 'en';
}

class I18n {
  lang = $state<Lang>('en');
  /** The messages of `lang`, switched together with it so the UI never shows a language half loaded. */
  dict = $state.raw<Dict>(en);

  /** Only the newest `load` may apply its result. */
  private seq = 0;

  get locale(): string {
    return LOCALES[this.lang];
  }

  /**
   * Switches the whole UI to a language. English applies synchronously; the others once their chunk has loaded. A
   * chunk that fails to load keeps the language on screen, and a newer choice always wins over an older one.
   */
  async load(lang: Lang): Promise<void> {
    this.seq += 1;
    const mine = this.seq;
    let dict: Dict = en;

    if (lang !== 'en') {
      try {
        dict = await LOADERS[lang]();
      } catch {
        return;
      }
    }

    if (mine !== this.seq) {
      return;
    }

    this.dict = dict;
    this.lang = lang;
    document.documentElement.lang = lang;
  }
}

export const i18n = new I18n();

export type Params = Record<string, string | number | null | undefined>;

function lookup(key: string): string | undefined {
  return i18n.dict[key] ?? (en as Record<string, string>)[key];
}

function fill(template: string, params?: Params): string {
  if (!params) {
    return template;
  }

  return template.replace(/\{(\w+)\}/g, (_, name: string) => {
    const value = params[name];

    return value === null || value === undefined ? '' : String(value);
  });
}

/** Translates a key, filling its `{name}` placeholders. Reactive on the current language. */
export function t(key: MessageKey, params?: Params): string {
  return fill(lookup(key) ?? key, params);
}

/** For keys built at run time (an id from the backend) that may not exist. */
export function tMaybe(key: string, params?: Params): string | null {
  const template = lookup(key);

  return template === undefined ? null : fill(template, params);
}

/** The keys that have plural forms: `chat.queued` for `chat.queued.one` and `chat.queued.other`. */
export type PluralBase = MessageKey extends infer Key ? (Key extends `${infer Base}.other` ? Base : never) : never;

const rules = new Map<string, Intl.PluralRules>();
const numbers = new Map<string, Intl.NumberFormat>();

function cached<T>(cache: Map<string, T>, locale: string, make: () => T): T {
  let value = cache.get(locale);

  if (!value) {
    value = make();
    cache.set(locale, value);
  }

  return value;
}

/**
 * A message with a count: picks `${base}.one`, `.few`, `.many`, ... by the language's plural rules, falls back to
 * `${base}.other`, and writes `{count}` grouped for the language. Other placeholders come from `params`.
 */
export function tn(base: PluralBase, count: number, params?: Params): string {
  const locale = i18n.locale;
  const category = cached(rules, locale, () => new Intl.PluralRules(locale)).select(count);
  const template = lookup(`${base}.${category}`) ?? lookup(`${base}.other`) ?? base;
  const grouped = cached(numbers, locale, () => new Intl.NumberFormat(locale)).format(count);

  return fill(template, { ...params, count: grouped });
}

export type { MessageKey };
