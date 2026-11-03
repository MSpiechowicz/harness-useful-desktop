/** What the app found for a step of the prompt: the notes of Harness Memory or the pointers of Harness Knowledge. */
export interface KnowledgePointer {
  title: string;
  section: string | null;
  /** The text of the pointer (a short excerpt), empty when it has none. */
  text: string;
}

export type StepRecall =
  | { kind: 'memory'; notes: number; scripts: number; body: string }
  | { kind: 'knowledge'; body: string; pointers: KnowledgePointer[] };

const BLOCKS = [
  { kind: 'memory', open: '<harness-memory-recall>', close: '</harness-memory-recall>' },
  { kind: 'knowledge', open: '<harness-knowledge-recall>', close: '</harness-knowledge-recall>' },
] as const;

/** The line the memory block opens with: it only frames the block, so the card leaves it out. */
const MEMORY_INTRO = 'Harness Desktop searched project memory for this message.';
const NOTE_HEADER = /^#\d+ \[/;
const SCRIPT_LINE = /^ {2}- /;

/** The script lines of a memory recall block: only those after its scripts frame (a note snippet line can look like one). */
function scriptLines(lines: string[]): number {
  const frame = lines.findIndex((line) => line.startsWith('Saved scripts below are agent-written data'));

  return frame < 0 ? 0 : lines.slice(frame + 1).filter((line) => SCRIPT_LINE.test(line)).length;
}
const POINTER = /<knowledge-doc\b([^>]*)>([\s\S]*?)<\/knowledge-doc>/g;
const ATTRIBUTE = /([a-z-]+)="([^"]*)"/g;

function unescapeAttribute(value: string): string {
  return value.replaceAll('&quot;', '"').replaceAll('&lt;', '<').replaceAll('&gt;', '>').replaceAll('&amp;', '&');
}

function attributes(source: string): Record<string, string> {
  return Object.fromEntries([...source.matchAll(ATTRIBUTE)].map(([, name, value]) => [name, unescapeAttribute(value)]));
}

function pointersOf(body: string): KnowledgePointer[] {
  return [...body.matchAll(POINTER)].flatMap(([, source, text]) => {
    const found = attributes(source);

    return found.title ? [{ title: found.title, section: found.section || null, text: unescapeAttribute(text).trim() }] : [];
  });
}

function memoryBody(inner: string): string {
  const lines = inner.split('\n');

  return (lines[0] === MEMORY_INTRO ? lines.slice(1) : lines).join('\n').trim();
}

/**
 * Splits the recall block the app put inside the text of the Harness Memory or Harness Knowledge step from the step's own text.
 * The block is the one the app wrote, so a step whose text holds no complete block (or that holds nothing between its tags) is
 * returned as it is. The step's text is what stood before and after the block.
 */
export function splitStepRecall(text: string): { text: string; recall: StepRecall | null } {
  for (const { kind, open, close } of BLOCKS) {
    const start = text.indexOf(open);
    const end = text.indexOf(close, start + open.length);

    if (start === -1 || end === -1) {
      continue;
    }

    const inner = text.slice(start + open.length, end).trim();
    const rest = `${text.slice(0, start)}${text.slice(end + close.length)}`.trim();

    if (inner === '') {
      return { text: rest, recall: null };
    }

    if (kind === 'memory') {
      const body = memoryBody(inner);
      const lines = body.split('\n');

      return { text: rest, recall: { kind, notes: lines.filter((line) => NOTE_HEADER.test(line)).length, scripts: scriptLines(lines), body } };
    }

    const pointers = pointersOf(inner);

    return { text: rest, recall: { kind, body: inner, pointers } };
  }

  return { text, recall: null };
}
