import { vi } from 'vitest';

const OS_LOCALE = 'pl-PL';

/**
 * Makes everything that formats "in the system language" behave as on a Polish system:
 * `toLocaleString()` without a locale, and `Intl` formatters created without one.
 * Code that names its locale is untouched. Undo with `vi.restoreAllMocks()` and `vi.unstubAllGlobals()`.
 */
export function simulatePolishOs(): void {
  const { NumberFormat, DateTimeFormat } = Intl;

  vi.spyOn(Number.prototype, 'toLocaleString').mockImplementation(function (this: number, locales, options) {
    return new NumberFormat(locales ?? OS_LOCALE, options).format(this);
  });
  vi.spyOn(Date.prototype, 'toLocaleString').mockImplementation(function (this: Date, locales, options) {
    return new DateTimeFormat(locales ?? OS_LOCALE, options ?? { dateStyle: 'long', timeStyle: 'medium' }).format(this);
  });

  // Intl's members are not enumerable, so the stand-in inherits them instead of spreading them.
  vi.stubGlobal('Intl', Object.assign(Object.create(Intl), {
    NumberFormat: class extends NumberFormat {
      constructor(locales?: string | string[], options?: Intl.NumberFormatOptions) {
        super(locales ?? OS_LOCALE, options);
      }
    },
    DateTimeFormat: class extends DateTimeFormat {
      constructor(locales?: string | string[], options?: Intl.DateTimeFormatOptions) {
        super(locales ?? OS_LOCALE, options);
      }
    },
  }));
}
