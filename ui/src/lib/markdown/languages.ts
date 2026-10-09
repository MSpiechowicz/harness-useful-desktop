import type { LanguageInput } from 'shiki/core';

type Loader = () => Promise<{ default: LanguageInput }>;

/** One lazy loader per grammar: each becomes its own chunk, fetched the first time a block needs it. */
const GRAMMARS = {
  typescript: () => import('shiki/langs/typescript.mjs'),
  tsx: () => import('shiki/langs/tsx.mjs'),
  javascript: () => import('shiki/langs/javascript.mjs'),
  jsx: () => import('shiki/langs/jsx.mjs'),
  json: () => import('shiki/langs/json.mjs'),
  rust: () => import('shiki/langs/rust.mjs'),
  python: () => import('shiki/langs/python.mjs'),
  shellscript: () => import('shiki/langs/shellscript.mjs'),
  toml: () => import('shiki/langs/toml.mjs'),
  yaml: () => import('shiki/langs/yaml.mjs'),
  markdown: () => import('shiki/langs/markdown.mjs'),
  diff: () => import('shiki/langs/diff.mjs'),
  svelte: () => import('shiki/langs/svelte.mjs'),
  html: () => import('shiki/langs/html.mjs'),
  css: () => import('shiki/langs/css.mjs'),
  go: () => import('shiki/langs/go.mjs'),
} satisfies Record<string, Loader>;

export type Grammar = keyof typeof GRAMMARS;

const ALIASES: Record<string, Grammar> = {
  ts: 'typescript',
  typescript: 'typescript',
  mts: 'typescript',
  cts: 'typescript',
  tsx: 'tsx',
  js: 'javascript',
  javascript: 'javascript',
  mjs: 'javascript',
  cjs: 'javascript',
  jsx: 'jsx',
  json: 'json',
  jsonc: 'json',
  json5: 'json',
  rust: 'rust',
  rs: 'rust',
  python: 'python',
  py: 'python',
  bash: 'shellscript',
  sh: 'shellscript',
  shell: 'shellscript',
  zsh: 'shellscript',
  console: 'shellscript',
  shellscript: 'shellscript',
  toml: 'toml',
  yaml: 'yaml',
  yml: 'yaml',
  md: 'markdown',
  markdown: 'markdown',
  diff: 'diff',
  patch: 'diff',
  svelte: 'svelte',
  html: 'html',
  htm: 'html',
  css: 'css',
  go: 'go',
  golang: 'go',
};

/** The bundled grammar a fence label maps to, or null when the block stays plain. */
export function grammarFor(lang: string): Grammar | null {
  return ALIASES[lang.trim().toLowerCase()] ?? null;
}

export function loadGrammar(grammar: Grammar): Promise<{ default: LanguageInput }> {
  return GRAMMARS[grammar]();
}
