import { afterEach, describe, expect, it, vi } from 'vitest';

import { i18n, LANGS, t, tMaybe, tn, toLang } from './i18n.svelte';

afterEach(() => {
  vi.doUnmock('./locales/de');
  vi.resetModules();
});

describe('plural forms', () => {
  it('uses English one and other', () => {
    expect(tn('shell.count.file', 1)).toBe('1 file');
    expect(tn('shell.count.file', 2)).toBe('2 files');
    expect(tn('shell.count.file', 0)).toBe('0 files');
  });

  it('uses the Polish one, few, many and other forms', async () => {
    await i18n.load('pl');

    expect(tn('shell.count.file', 1)).toBe('1 plik');
    expect(tn('shell.count.file', 2)).toBe('2 pliki');
    expect(tn('shell.count.file', 5)).toBe('5 plików');
    expect(tn('shell.count.file', 12)).toBe('12 plików');
    expect(tn('shell.count.file', 22)).toBe('22 pliki');
    expect(tn('shell.count.file', 1.5)).toBe('1,5 pliku');
  });

  it('uses the Brazilian Portuguese one and other forms, with zero and one singular', async () => {
    await i18n.load('pt');

    expect(i18n.locale).toBe('pt-BR');
    expect(tn('shell.count.file', 0)).toBe('0 arquivo');
    expect(tn('shell.count.file', 1)).toBe('1 arquivo');
    expect(tn('shell.count.file', 2)).toBe('2 arquivos');
    expect(tn('shell.count.file', 17_612)).toBe('17.612 arquivos');
  });

  it('groups the count the way the language does', async () => {
    expect(tn('shell.count.file', 17_612)).toBe('17,612 files');

    await i18n.load('pl');

    expect(tn('shell.count.file', 17_612)).toBe('17\u00a0612 plików');
  });
});

describe('messages', () => {
  it('fills placeholders and leaves a missing one empty', () => {
    expect(t('shell.palette.goTo', { view: 'Chat' })).toBe('Go to Chat');
    expect(t('shell.palette.goTo', {})).toBe('Go to ');
  });

  it('answers a dynamic key only when it exists', () => {
    expect(tMaybe('shell.view.chat')).toBe('Chat');
    expect(tMaybe('shell.view.nothing')).toBeNull();
  });

  it('switches every message at once and sets the document language', async () => {
    await i18n.load('de');

    expect(t('common.cancel')).toBe('Abbrechen');
    expect(i18n.lang).toBe('de');
    expect(i18n.locale).toBe('de-DE');
    expect(document.documentElement.lang).toBe('de');

    await i18n.load('en');

    expect(t('common.cancel')).toBe('Cancel');
    expect(document.documentElement.lang).toBe('en');
  });
});

describe('Brazilian Portuguese', () => {
  it('is offered under its own name and switches the messages', async () => {
    expect(LANGS.find((lang) => lang.code === 'pt')?.label).toBe('Português (Brasil)');

    await i18n.load('pt');

    expect(t('common.cancel')).toBe('Cancelar');
    expect(t('shell.palette.goTo', { view: 'Chat' })).toBe('Ir para Chat');
    expect(i18n.lang).toBe('pt');
  });
});

describe('loading', () => {
  it('lets the newest choice win over a slower earlier one', async () => {
    const slow = i18n.load('pl');
    const fast = i18n.load('es');
    await Promise.all([slow, fast]);

    expect(i18n.lang).toBe('es');
    expect(t('common.cancel')).toBe('Cancelar');
  });

  it('applies a choice of English at once, dropping a language still loading', async () => {
    const pending = i18n.load('fr');
    void i18n.load('en');
    await pending;

    expect(i18n.lang).toBe('en');
  });

  it('keeps the language on screen when a language file fails to load', async () => {
    vi.resetModules();
    vi.doMock('./locales/de', () => {
      throw new Error('chunk missing');
    });
    const fresh = await import('./i18n.svelte');
    await fresh.i18n.load('pl');

    await fresh.i18n.load('de');

    expect(fresh.i18n.lang).toBe('pl');
    expect(fresh.t('common.cancel')).toBe('Anuluj');
  });
});

describe('toLang', () => {
  it('accepts every supported language', () => {
    for (const lang of LANGS) {
      expect(toLang(lang.code)).toBe(lang.code);
    }
  });

  it('turns anything else into English', () => {
    expect(toLang('xx')).toBe('en');
    expect(toLang('PL')).toBe('en');
    expect(toLang('')).toBe('en');
    expect(toLang(undefined)).toBe('en');
    expect(toLang(null)).toBe('en');
  });
});
