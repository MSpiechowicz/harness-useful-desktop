import type { FactRow } from '$lib/components/facts';
import { type MessageKey, t } from '$lib/i18n/i18n.svelte';
import type { ScriptSummary } from '$lib/ipc/bindings/ScriptSummary';

/** What a script's interpreter is called on screen. These are product names and stay the same in every language. */
const LANGUAGE_LABELS: Record<string, string> = { sh: 'sh', bash: 'Bash', python: 'Python', node: 'Node.js', bun: 'Bun' };

/** The grammar the code block highlights a script with. */
const GRAMMARS: Record<string, string> = { sh: 'sh', bash: 'bash', python: 'python', node: 'js', bun: 'ts' };

export function languageLabel(language: string): string {
  return LANGUAGE_LABELS[language] ?? language;
}

export function grammarOf(language: string): string {
  return GRAMMARS[language] ?? '';
}

export const PLACE_LABELS = {
  team: 'memory.scripts.place.team',
  local: 'memory.scripts.place.local',
} as const satisfies Record<ScriptSummary['place'], MessageKey>;

export const APPROVAL_LABELS = {
  local: 'memory.scripts.approval.local',
  approved: 'memory.scripts.approval.approved',
  team: 'memory.scripts.approval.team',
  needed: 'memory.scripts.approval.needed',
} as const satisfies Record<ScriptSummary['approval'], MessageKey>;

/** Whether agents are held back from running a script until the user trusts it. */
export function needsReview(script: Pick<ScriptSummary, 'approval'>): boolean {
  return script.approval === 'needed';
}

/** The first characters of the body's hash: enough to tell two versions apart. */
export function shortSha(sha: string): string {
  return sha.slice(0, 8);
}

/** Whether a script answers a search: by name, description, language or repository. */
export function matchesQuery(script: ScriptSummary, query: string): boolean {
  const needle = query.trim().toLowerCase();

  if (needle === '') {
    return true;
  }

  return [script.name, script.description, languageLabel(script.language), script.repo ?? ''].some((field) => field.toLowerCase().includes(needle));
}

export type ExitOutcome = 'none' | 'ok' | 'failed';

/** How the last run ended: not at all (never run, or killed without a code), well, or badly. */
export function exitOutcome(code: number | null): ExitOutcome {
  if (code === null) {
    return 'none';
  }

  return code === 0 ? 'ok' : 'failed';
}

/** Where a script applies: everywhere (global), its repository, or a neutral label for a repository script that names none. */
export function scopeText(script: Pick<ScriptSummary, 'scope' | 'repo'>): string {
  if (script.scope !== 'repo') {
    return t('memory.scripts.row.global');
  }

  return script.repo?.trim() ? script.repo : t('memory.scripts.row.thisRepo');
}

/** Who wrote a script: the harness, and the project it was written in when that is known. */
export function authorText(script: Pick<ScriptSummary, 'author'>): string {
  return script.author.root ? `${script.author.source} · ${script.author.root}` : script.author.source;
}

const HOME_FOLDER = /^(?:\/home\/[^/]+|\/Users\/[^/]+|[A-Za-z]:[\\/]Users[\\/][^\\/]+)(?=$|[\\/])/;

/** A folder as people write it: the home folder is a tilde, so what is left (the project) is what the eye lands on. */
export function tildePath(path: string): string {
  return path.replace(HOME_FOLDER, '~');
}

/** The two lines of a Details list about who wrote an item: the client, and the folder it was written in when that is known. */
export function authorFacts(item: { author: { source: string; root: string | null } }): FactRow[] {
  const { source, root } = item.author;
  const who = { label: t('memory.detail.author'), value: source };

  return root ? [who, { label: t('memory.detail.folder'), value: tildePath(root), title: root, cutStart: true }] : [who];
}

const SAFE_ARGUMENT = /^[\w@%+=:,./-]+$/;

/** An argument as a shell would need it written, so the line shown is the one an agent's arguments make: single quotes around anything but plain characters. */
export function shellQuote(argument: string): string {
  if (SAFE_ARGUMENT.test(argument)) {
    return argument;
  }

  return `'${argument.replaceAll("'", "'\\''")}'`;
}

export interface BodySegment {
  text: string;
  /** True for a character that cannot be seen in code: `text` is then its escape, such as `⟨U+202E⟩`. */
  hidden: boolean;
  /** Set on the escape of a lone carriage return: a terminal or an editor starts a new line there, so the view does too. */
  lineBreak?: true;
}

const ZERO_WIDTH_JOINER = 0x200d;
const TEXT_SELECTOR = 0xfe0e;
const EMOJI_SELECTOR = 0xfe0f;
const CARRIAGE_RETURN = 0x0d;
const LINE_FEED = 0x0a;
const TAB = 0x09;

type Range = readonly [number, number];

/** Invisible characters, or ones that change how the text around them is shown. A copy of the Memory server's table (`FORMAT_CHARACTERS`), kept in step by `hidden-chars.fixture.json`. */
const FORMAT_RANGES: readonly Range[] = [
  [0x00ad, 0x00ad],
  [0x034f, 0x034f],
  [0x0600, 0x0605],
  [0x061c, 0x061c],
  [0x06dd, 0x06dd],
  [0x070f, 0x070f],
  [0x0890, 0x0891],
  [0x08e2, 0x08e2],
  [0x115f, 0x1160],
  [0x17b4, 0x17b5],
  [0x180b, 0x180f],
  [0x200b, 0x200f],
  [0x2028, 0x202e],
  [0x2060, 0x2064],
  [0x2066, 0x206f],
  [0x2800, 0x2800],
  [0x3164, 0x3164],
  [0xfe00, 0xfe0f],
  [0xfeff, 0xfeff],
  [0xffa0, 0xffa0],
  [0xfff9, 0xfffb],
  [0x110bd, 0x110bd],
  [0x110cd, 0x110cd],
  [0x13430, 0x1343f],
  [0x1bca0, 0x1bca3],
  [0x1d173, 0x1d17a],
  [0xe0001, 0xe0001],
  [0xe0020, 0xe007f],
  [0xe0100, 0xe01ef],
];

/** Symbols an emoji presentation, a text presentation or a joiner legitimately follows. Mirrors the Memory server's `is_emoji_symbol`. */
const EMOJI_SYMBOL_RANGES: readonly Range[] = [
  [0x00a9, 0x00a9],
  [0x00ae, 0x00ae],
  [0x203c, 0x203c],
  [0x2049, 0x2049],
  [0x2122, 0x2122],
  [0x2139, 0x2139],
  [0x2190, 0x21ff],
  [0x2300, 0x23ff],
  [0x2460, 0x27bf],
  [0x2900, 0x297f],
  [0x2b00, 0x2bff],
  [0x3030, 0x3030],
  [0x303d, 0x303d],
  [0x3297, 0x3297],
  [0x3299, 0x3299],
  [0x1f000, 0x1faff],
];

function inRanges(ranges: readonly Range[], code: number | undefined): boolean {
  return code !== undefined && ranges.some(([from, to]) => code >= from && code <= to);
}

/** Digits, `#` and `*` start a keycap emoji when the emoji selector follows them. */
function isKeycapBase(code: number | undefined): boolean {
  return code !== undefined && ((code >= 0x30 && code <= 0x39) || code === 0x23 || code === 0x2a);
}

function isControl(code: number): boolean {
  return code <= 0x1f || (code >= 0x7f && code <= 0x9f);
}

/** Mirrors the Memory server's `is_hidden`, so what the dialog flags is exactly what the server refuses to run. */
function isHiddenAt(codes: number[], index: number): boolean {
  const code = codes[index];
  const before = codes[index - 1];
  const after = codes[index + 1];
  const afterEmoji = inRanges(EMOJI_SYMBOL_RANGES, before);

  if (code === LINE_FEED || code === TAB) {
    return false;
  }

  if (code === CARRIAGE_RETURN) {
    return after !== LINE_FEED;
  }

  if (code === ZERO_WIDTH_JOINER) {
    return !(afterEmoji || inRanges(EMOJI_SYMBOL_RANGES, after));
  }

  if (code === TEXT_SELECTOR) {
    return !afterEmoji;
  }

  if (code === EMOJI_SELECTOR) {
    return !(afterEmoji || isKeycapBase(before));
  }

  return isControl(code) || inRanges(FORMAT_RANGES, code);
}

function escapeCode(code: number): string {
  return `⟨U+${code.toString(16).toUpperCase().padStart(4, '0')}⟩`;
}

/** Splits a body into plain text and escaped invisible characters; `count` is how many were found. */
export function revealHidden(body: string): { segments: BodySegment[]; count: number } {
  const codes = Array.from(body, (char) => char.codePointAt(0) as number);
  const segments: BodySegment[] = [];
  let count = 0;
  let plain = '';

  codes.forEach((code, index) => {
    if (!isHiddenAt(codes, index)) {
      plain += String.fromCodePoint(code);

      return;
    }

    if (plain !== '') {
      segments.push({ text: plain, hidden: false });
      plain = '';
    }

    segments.push(code === CARRIAGE_RETURN ? { text: escapeCode(code), hidden: true, lineBreak: true } : { text: escapeCode(code), hidden: true });
    count += 1;
  });

  if (plain !== '') {
    segments.push({ text: plain, hidden: false });
  }

  return { segments, count };
}

/** Text with each invisible character spelled out, for places that cannot show a chip: a title, a description. */
export function spelledOut(text: string): string {
  return revealHidden(text)
    .segments.map((part) => part.text)
    .join('');
}
