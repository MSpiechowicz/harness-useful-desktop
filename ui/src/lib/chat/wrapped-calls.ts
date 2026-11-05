import type { JsonValue } from '$lib/ipc/bindings/serde_json/JsonValue';

import { blankToolCall, isJsonObject } from './tools';
import type { ToolCallState } from './types';

/**
 * OMP agents do not call the Harness family tools as tools of their own, so the chat sees another tool's name. They mostly write the
 * arguments to the tool's own address, a `write` call with the path `xd://mcp__harness_code_code_map`, and the reply is the whole text.
 * Or they run them from a Python script of the `eval` tool, often several in one script, with `tool.mcp__harness_code_code_map({...})`
 * in its code and what it printed as its result. This turns such a call into the calls it made, named as a direct call is named, so
 * whatever reads family calls reads these too. The ids of a script's calls are `<eval id>#<n>`.
 *
 * What a script prints is up to the agent. The replies are told apart by a `RESULT n:` line (the steps intro asks for it) or by the
 * `{'text': …}` line Python prints for a whole result. When they cannot be matched to the calls one by one, nothing is expanded.
 *
 * OMP cuts an output line at 768 bytes, and a whole result is one line, so a reply printed that way may be only its start: what it
 * lost is unknown here, and a card shows what is left.
 */
const FAMILY_ADDRESS = /^xd:\/\/mcp__harness_(memory|knowledge|code)_([a-z][a-z0-9_]*)$/;
const FAMILY_CALL = /\btool\.mcp__harness_(memory|knowledge|code)_([a-z][a-z0-9_]*)\(/g;
const RESULT_HEADER = /^RESULT \d+:?\s*$/m;
const REPR_START = "{'text': ";
const CUT_NOTICE = /^\[Some lines truncated.*$/gm;

const ESCAPES: Record<string, string> = { n: '\n', t: '\t', r: '\r', '\\': '\\', "'": "'", '"': '"' };
const HEX_DIGITS: Record<string, number> = { x: 2, u: 4, U: 8 };

/** A Python string literal from `start`, its opening quote; `closed` is false when the output was cut inside it. */
function pythonString(source: string, start: number): { text: string; end: number; closed: boolean } {
  const quote = source[start];
  let text = '';
  let index = start + 1;

  while (index < source.length) {
    const char = source[index];

    if (char === quote) {
      return { text, end: index + 1, closed: true };
    }

    if (char !== '\\') {
      text += char;
      index += 1;
      continue;
    }

    const next = source[index + 1] ?? '';
    const digits = HEX_DIGITS[next];

    if (digits) {
      const code = Number.parseInt(source.slice(index + 2, index + 2 + digits), 16);

      text += Number.isNaN(code) || code > 0x10ffff ? '' : String.fromCodePoint(code);
      index += 2 + digits;
    } else {
      text += ESCAPES[next] ?? next;
      index += 2;
    }
  }

  return { text, end: source.length, closed: false };
}

/** A reply as a script printed it: a whole result, of which a cut one loses its last line as it may stop inside a word, or plain text. */
function replyOf(section: string): { text: string; failed: boolean } {
  const printed = section.trim();

  if (!printed.startsWith(REPR_START)) {
    return { text: printed, failed: false };
  }

  const { text, end, closed } = pythonString(printed, REPR_START.length);

  if (!closed) {
    return { text: text.slice(0, Math.max(text.lastIndexOf('\n'), 0)), failed: false };
  }

  return { text, failed: /'isError': True/.test(printed.slice(end)) };
}

/** What the script printed, one piece for each reply; null when it cannot be told apart. */
function sectionsOf(output: string, count: number): string[] | null {
  const clean = output.replace(CUT_NOTICE, '');

  if (RESULT_HEADER.test(clean)) {
    const [, ...sections] = clean.split(new RegExp(RESULT_HEADER, 'gm'));

    return sections.length === count ? sections : null;
  }

  const lines = clean.split('\n').filter((line) => line.startsWith(REPR_START));

  if (lines.length === count) {
    return lines;
  }

  return count === 1 && lines.length === 0 ? [clean] : null;
}

/** The first argument of a call in the script, when it is written as JSON (a query is what a card shows of it). */
function argsAt(code: string, start: number): JsonValue | null {
  if (code[start] !== '{') {
    return null;
  }

  let depth = 0;
  let quote: string | null = null;

  for (let index = start; index < code.length; index += 1) {
    const char = code[index];

    if (quote) {
      if (char === '\\') {
        index += 1;
      } else if (char === quote) {
        quote = null;
      }
    } else if (char === '"' || char === "'") {
      quote = char;
    } else if (char === '{') {
      depth += 1;
    } else if (char === '}' && (depth -= 1) === 0) {
      try {
        return JSON.parse(code.slice(start, index + 1)) as JsonValue;
      } catch {
        return null;
      }
    }
  }

  return null;
}

const expanded = new WeakMap<ToolCallState, ToolCallState[]>();

/** A `write` call to a family tool's address: its content is the arguments, written as JSON. */
function writtenCall(call: ToolCallState, path: string, content: JsonValue | undefined): ToolCallState[] {
  const address = FAMILY_ADDRESS.exec(path);

  if (!address) {
    return [call];
  }

  let args: JsonValue | null = null;

  try {
    args = typeof content === 'string' ? (JSON.parse(content) as JsonValue) : (content ?? null);
  } catch {
    args = null;
  }

  return [{ ...call, name: `mcp__harness-${address[1]}__${address[2]}`, args }];
}

function expand(call: ToolCallState): ToolCallState[] {
  const fields = isJsonObject(call.args) ? call.args : {};

  if (call.name === 'write' && typeof fields.path === 'string') {
    return writtenCall(call, fields.path, fields.content);
  }

  const { code } = fields;

  if (call.name !== 'eval' || typeof code !== 'string' || call.resultText === null || call.status === 'running' || call.status === 'pending') {
    return [call];
  }

  const made = [...code.matchAll(FAMILY_CALL)];
  const sections = made.length > 0 ? sectionsOf(call.resultText, made.length) : null;

  if (!sections) {
    return [call];
  }

  return made.map((match, index) => {
    const { text, failed } = replyOf(sections[index]);

    return {
      ...blankToolCall(`${call.id}#${index}`, `mcp__harness-${match[1]}__${match[2]}`),
      args: argsAt(code, match.index + match[0].length),
      resultText: text,
      status: failed ? 'error' : 'ok',
    };
  });
}

/** The calls a call stands for: itself, unless it is a way of running Harness family tools. A chat is read on every delta, so this is kept per call. */
export function expandWrapped(call: ToolCallState): ToolCallState[] {
  const known = expanded.get(call);

  if (known) {
    return known;
  }

  const calls = expand(call);

  expanded.set(call, calls);

  return calls;
}
