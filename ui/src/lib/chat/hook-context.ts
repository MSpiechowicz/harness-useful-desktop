const CONTEXT_OPEN = '<harness-hook-context>\n';
const CONTEXT_CLOSE = '</harness-hook-context>';
const ELEMENT_OPEN = '<hook name="';
const ELEMENT_CLOSE = '</hook>';
const FOLLOW_UP_OPEN = '<harness-hook-followup name="';
const FOLLOW_UP_CLOSE = '</harness-hook-followup>';

/** The name the app gives its own part (the chat's Memory settings told to the agent), as `MEMORY_NOTICE_NAME` in the hooks engine writes it. */
export const MEMORY_SETTINGS_PART = 'Memory settings';

/** What one hook put in a prompt: its name and its text, unescaped. */
export interface HookText {
  name: string;
  text: string;
}

/** The markers a `<` in a hook's text must not begin, which the app writes as `&lt;` (lowercase, without the `<`). */
const MARKERS = ['hook', '/hook', 'harness-hook', '/harness-hook'];

function beginsMarker(rest: string): boolean {
  const head = rest.toLowerCase();

  return MARKERS.some((marker) => head.startsWith(marker));
}

/** A name as the app wrote it: `& < > "` are escaped. */
function unescapeName(name: string): string {
  return name.replaceAll('&quot;', '"').replaceAll('&gt;', '>').replaceAll('&lt;', '<').replaceAll('&amp;', '&');
}

/** A text as the app wrote it: only a `<` that begins a hook marker is written as `&lt;`, every other `&lt;` is the text's own. */
function unescapeText(text: string): string {
  let out = '';
  let rest = text;
  let at = rest.indexOf('&lt;');

  while (at !== -1) {
    const after = rest.slice(at + 4);

    out += rest.slice(0, at) + (beginsMarker(after) ? '<' : '&lt;');
    rest = after;
    at = rest.indexOf('&lt;');
  }

  return out + rest;
}

/** One element `<tag name="NAME">TEXT</tag>` at the start of `from`; the text after it, or null when it is not well formed. */
function takeElement(from: string, open: string, close: string): { item: HookText; rest: string } | null {
  if (!from.startsWith(open)) {
    return null;
  }

  const afterOpen = from.slice(open.length);
  const nameEnd = afterOpen.indexOf('"');

  if (nameEnd === -1 || !afterOpen.slice(nameEnd).startsWith('">')) {
    return null;
  }

  const body = afterOpen.slice(nameEnd + 2);
  const textEnd = body.indexOf(close);

  if (textEnd === -1) {
    return null;
  }

  const item = { name: unescapeName(afterOpen.slice(0, nameEnd)), text: unescapeText(body.slice(0, textEnd)) };

  return { item, rest: body.slice(textEnd + close.length) };
}

/**
 * Splits the hook context the app puts in front of a prompt from the user's own text. Mirrors the backend's strict
 * `parse_hook_context`: the block must start the message, hold one or more `<hook name="…">…</hook>` elements each on
 * its own line, close with `</harness-hook-context>` and be followed by a blank line or nothing. Anything else (a forged or
 * half-written block) is plain text.
 */
export function splitHookContext(message: string): { hooks: HookText[]; text: string } {
  const plain = { hooks: [], text: message };

  if (!message.startsWith(CONTEXT_OPEN)) {
    return plain;
  }

  const hooks: HookText[] = [];
  let rest = message.slice(CONTEXT_OPEN.length);

  for (;;) {
    const taken = takeElement(rest, ELEMENT_OPEN, ELEMENT_CLOSE);

    if (!taken || !taken.rest.startsWith('\n')) {
      return plain;
    }

    hooks.push(taken.item);
    rest = taken.rest.slice(1);

    if (rest.startsWith(CONTEXT_CLOSE)) {
      rest = rest.slice(CONTEXT_CLOSE.length);
      break;
    }
  }

  if (rest === '') {
    return { hooks, text: '' };
  }

  return rest.startsWith('\n\n') ? { hooks, text: rest.slice(2) } : plain;
}

/**
 * The hooks that sent a message, when the whole message is the follow-up a hook sent: one or more
 * `<harness-hook-followup name="…">…</harness-hook-followup>` elements joined by a blank line and nothing else.
 * Anything else is the user's own message and gives null.
 */
export function splitHookFollowUp(message: string): HookText[] | null {
  const hooks: HookText[] = [];
  let rest = message;

  for (;;) {
    const taken = takeElement(rest, FOLLOW_UP_OPEN, FOLLOW_UP_CLOSE);

    if (!taken) {
      return null;
    }

    hooks.push(taken.item);

    if (taken.rest === '') {
      return hooks;
    }

    if (!taken.rest.startsWith('\n\n')) {
      return null;
    }

    rest = taken.rest.slice(2);
  }
}

/** What a user message holds: the user's own text, the context hooks put in front of it, or the hooks that sent it. */
export interface HookSplit {
  /** The text the user wrote; empty for a message a hook sent. */
  text: string;
  /** The hooks that added context in front of the user's text. */
  context: HookText[];
  /** The hooks that sent the whole message. */
  followUp: HookText[];
}

export function splitHooks(message: string): HookSplit {
  const followUp = splitHookFollowUp(message);

  if (followUp) {
    return { text: '', context: [], followUp };
  }

  const { hooks, text } = splitHookContext(message);

  return { text, context: hooks, followUp: [] };
}
