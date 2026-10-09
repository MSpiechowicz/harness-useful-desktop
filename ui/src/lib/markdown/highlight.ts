import type { HighlighterCore, ThemedToken } from 'shiki/core';

import { grammarFor, loadGrammar, type Grammar } from './languages';

/** Highlighted code: lines of colored runs. */
export type HighlightedLine = { content: string; color?: string; bold: boolean; italic: boolean }[];

const THEME = 'harness';
/** Very large blocks stay plain; tokenizing them would stall the UI for no reader benefit. */
const MAX_CHARS = 60_000;

let highlighter: Promise<HighlighterCore> | undefined;
const loaded = new Set<Grammar>();

/**
 * One shared highlighter, created on first use. The JavaScript regex engine keeps the CSP
 * free of wasm; the theme reads `--shiki-*` custom properties, so dark and light follow the app.
 */
function getHighlighter(): Promise<HighlighterCore> {
  highlighter ??= (async () => {
    const [{ createHighlighterCore, createCssVariablesTheme }, { createJavaScriptRegexEngine }] = await Promise.all([
      import('shiki/core'),
      import('shiki/engine/javascript'),
    ]);

    return createHighlighterCore({
      themes: [createCssVariablesTheme({ name: THEME, variablePrefix: '--shiki-', fontStyle: true })],
      langs: [],
      engine: createJavaScriptRegexEngine(),
    });
  })();

  return highlighter;
}

function lineOf(tokens: ThemedToken[]): HighlightedLine {
  // Shiki's font style is a bit mask: 1 italic, 2 bold.
  return tokens.map((token) => ({
    content: token.content,
    color: token.color,
    italic: ((token.fontStyle ?? 0) & 1) !== 0,
    bold: ((token.fontStyle ?? 0) & 2) !== 0,
  }));
}

/** Highlights `code`, or returns null when the language is unknown, the block is huge, or shiki fails. */
export async function highlight(code: string, lang: string): Promise<HighlightedLine[] | null> {
  const grammar = grammarFor(lang);
  if (grammar === null || code.length > MAX_CHARS) {
    return null;
  }

  try {
    const engine = await getHighlighter();

    if (!loaded.has(grammar)) {
      await engine.loadLanguage((await loadGrammar(grammar)).default);
      loaded.add(grammar);
    }

    return engine.codeToTokensBase(code, { lang: grammar, theme: THEME }).map(lineOf);
  } catch {
    return null;
  }
}
