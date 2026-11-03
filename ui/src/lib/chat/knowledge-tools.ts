import type { JsonValue } from '$lib/ipc/bindings/serde_json/JsonValue';

import { displayText } from '$lib/utils/display-text';
import { knownFormat } from '$lib/utils/knowledge-formats';

import { harnessToolName, MAX_CARD_ROWS, parseMarkedReply } from './harness-tools';
import { isJsonObject } from './tools';

/** The harness-knowledge tool a call name is (`knowledge_search` for `mcp__harness-knowledge__knowledge_search`, whatever the harness calls it), else null: see `harnessToolName`. */
export function knowledgeToolName(name: string): string | null {
  return harnessToolName('knowledge', name);
}

/** A document a reply names: `id` is the document as the server writes it (`12`, or `local:12` for this computer's store), never a chunk's. */
export interface FoundDoc {
  id: string;
  title: string;
  /** The headings above the chunk a search hit, `Guide > Install`; null for a reply that names none. */
  section: string | null;
  /** The format a `knowledge_get` reply names (`markdown`), when it is one the Knowledge view knows; null for a search, which does not say. */
  format: string | null;
}

export { MAX_CARD_ROWS };

/** The longest title and section a card shows, in characters. */
const MAX_TITLE_CHARS = 300;
const MAX_SECTION_CHARS = 400;

/** A document id or a chunk id as the server writes them (`12`, `12:340`, `local:12`, `local:12:340`). */
const REFERENCE = /^(local:)?(\d+)(?::(\d+))?$/;

/** The document a written id names: a chunk id stands for its document (`12:340` is `12`). Null when the text is not an id. */
export function docRef(raw: string): string | null {
  const match = REFERENCE.exec(raw.trim());
  if (!match || !Number.isSafeInteger(Number(match[2])) || (match[3] !== undefined && !Number.isSafeInteger(Number(match[3])))) {
    return null;
  }

  return `${match[1] ?? ''}${Number(match[2])}`;
}

/** Whether the text of a search reply says nothing matched (`no matching documents`) or there is nothing to match (`no documents yet`). */
export function saysNothing(text: string | null): boolean {
  return /no matching documents|no documents yet/i.test(text ?? '');
}

// `<knowledge-doc id="12:340" title="…" section="…" score="812">` of a search, and `<knowledge-doc id="12" title="…" format="markdown" chunks="3" updated="…" repos="…">`
// of a read: a line of its own, every value on one line with `&`, `<`, `>` and `"` written as entities. A document's own text cannot open another block
// (the server writes a `<` that starts one of its tags as `&lt;`), so only a header starts a line this way.
const HEADER = /^<knowledge-doc((?: [a-z]+="[^"]*")+)>$/;
const ATTRIBUTE = / ([a-z]+)="([^"]*)"/g;
const ENTITIES: Record<string, string> = { lt: '<', gt: '>', quot: '"', amp: '&' };

function unescapeValue(value: string): string {
  return value.replace(/&(lt|gt|quot|amp);/g, (_, name: string) => ENTITIES[name]);
}

/** The documents a reply shows the header of, in order, each once (a search can show several chunks of one), at most `MAX_CARD_ROWS`. A header that is not read in full is left out, never guessed. Titles and sections are shown without hidden characters. */
function headers(text: string | null): FoundDoc[] {
  const docs = new Map<string, FoundDoc>();

  for (const line of (text ?? '').split('\n')) {
    const attributes = HEADER.exec(line)?.[1];
    if (attributes === undefined) {
      continue;
    }

    const values = new Map([...attributes.matchAll(ATTRIBUTE)].map((match) => [match[1], unescapeValue(match[2])] as const));
    const id = docRef(values.get('id') ?? '');
    const title = values.get('title');

    if (id !== null && title !== undefined && !docs.has(id)) {
      docs.set(id, { id, title: displayText(title, MAX_TITLE_CHARS), section: displayText(values.get('section') ?? '', MAX_SECTION_CHARS) || null, format: knownFormat(values.get('format')) });
    }

    if (docs.size >= MAX_CARD_ROWS) {
      break;
    }
  }

  return [...docs.values()];
}

/** The documents a `knowledge_search` reply lists. The frame, the notices, `no matching documents`, the `… (n more)` trailer and the hint to open one are not rows. */
export function parseDocs(text: string | null): FoundDoc[] {
  return headers(text);
}

/** The opening tag of a pointer in the recall block the app puts in a prompt: its excerpt follows on the same line, so the tag does not stand alone. */
const POINTER = /<knowledge-doc((?: [a-z]+="[^"]*")+)>/g;

/** The documents a recall block points to, in order, each once (a document can have several chunks), at most `MAX_CARD_ROWS`. A pointer that is not read in full is left out. */
export function parseRecallDocs(text: string | null): FoundDoc[] {
  const docs = new Map<string, FoundDoc>();

  for (const [, attributes] of (text ?? '').matchAll(POINTER)) {
    const values = new Map([...attributes.matchAll(ATTRIBUTE)].map((match) => [match[1], unescapeValue(match[2])] as const));
    const id = docRef(values.get('id') ?? '');
    const title = values.get('title');

    if (id !== null && title !== undefined && !docs.has(id) && docs.size < MAX_CARD_ROWS) {
      docs.set(id, { id, title: displayText(title, MAX_TITLE_CHARS), section: displayText(values.get('section') ?? '', MAX_SECTION_CHARS) || null, format: null });
    }
  }

  return [...docs.values()];
}

/** The documents a `knowledge_get` reply shows: the block of the document or chunk read. */
export function parseOpenedDocs(text: string | null): FoundDoc[] {
  return headers(text);
}

/**
 * The documents a `knowledge_feedback` reply says it marked useful: `marked 2 documents useful: 12, local:3`, `marked 1 document useful: 12`
 * or `marked no documents useful`, then optional `not found: …` and `not checked: …` lines for ids that do not count. Null when the reply is not that.
 */
export function parseMarkedDocs(text: string | null): string[] | null {
  return parseMarkedReply(text, 'document', '(?:local:)?\\d+', docRef);
}

/** The documents a call names in `ids` and the scalar `id`, each once, at most `MAX_CARD_ROWS`: a chunk id stands for its document. */
export function docRefsOf(args: JsonValue | null): string[] {
  if (!isJsonObject(args)) {
    return [];
  }

  const named = [...(Array.isArray(args.ids) ? args.ids : []), args.id];
  const refs = named.flatMap((value) => (typeof value === 'string' || typeof value === 'number' ? [docRef(String(value))] : []));

  return [...new Set(refs.filter((ref) => ref !== null))].slice(0, MAX_CARD_ROWS);
}

/** The documents a `knowledge_feedback` call rated: the ones its reply confirms, else the ones it asked for. */
export function ratedDocs(args: JsonValue | null, resultText: string | null): string[] {
  return parseMarkedDocs(resultText) ?? docRefsOf(args);
}

/** The documents a `knowledge_get` call read: the one its reply shows, else the ones it asked for (a call that is still going has only the second). */
export function readDocs(args: JsonValue | null, resultText: string | null): string[] {
  const shown = parseOpenedDocs(resultText).map((doc) => doc.id);

  return shown.length > 0 ? shown : docRefsOf(args);
}
