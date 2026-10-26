/** A run of the user's own words, or a fenced code block that deserves the code component. */
export type TextPart = { type: 'text'; text: string } | { type: 'code'; lang: string; code: string };

const OPEN = /^ {0,3}(`{3,}|~{3,})[ \t]*([^\s`]*)[^`]*$/;

/**
 * Splits plain message text at its closed ``` / ~~~ fences. Everything else (headings, lists, line breaks) stays as typed,
 * so only code is rendered, not the whole message as Markdown. A fence that never closes stays text.
 */
export function splitFences(source: string): TextPart[] {
  const lines = source.split('\n');
  const parts: TextPart[] = [];
  let text: string[] = [];
  let index = 0;

  const flush = (): void => {
    const joined = text.join('\n').replace(/^\n+|\n+$/g, '');

    if (joined.trim() !== '') {
      parts.push({ type: 'text', text: joined });
    }

    text = [];
  };

  while (index < lines.length) {
    const open = OPEN.exec(lines[index]);
    const close = open ? closingLine(lines, index + 1, open[1]) : -1;

    if (open && close !== -1) {
      flush();
      parts.push({ type: 'code', lang: open[2], code: lines.slice(index + 1, close).join('\n') });
      index = close + 1;
    } else {
      text.push(lines[index]);
      index += 1;
    }
  }

  flush();

  return parts;
}

function closingLine(lines: string[], from: number, fence: string): number {
  for (let index = from; index < lines.length; index += 1) {
    const line = lines[index].trim();

    if (line.length >= fence.length && line[0] === fence[0] && [...line].every((char) => char === fence[0])) {
      return index;
    }
  }

  return -1;
}
