import type { MessageKey } from './en';

/** The plural categories English does not need but other languages may add next to `.other`. */
type ExtraPluralForm = 'zero' | 'two' | 'few' | 'many';

/**
 * The messages of one area in a language other than English: exactly the English keys, plus the optional extra plural
 * forms (`.zero`, `.two`, `.few`, `.many`) next to every `.other`. A missing or misspelled key fails `bun run check`.
 */
export type AreaMessages<T extends Record<string, string>> = { [K in keyof T]: string } & {
  [K in keyof T & string as K extends `${infer Base}.other` ? `${Base}.${ExtraPluralForm}` : never]?: string;
};

/** A whole language: every English key, plus the extra plural forms. */
export type Dict = Record<MessageKey, string> & Partial<Record<string, string>>;
