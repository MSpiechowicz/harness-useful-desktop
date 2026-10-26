import type { NoteContent } from '../bindings/NoteContent';
import type { NoteDetail } from '../bindings/NoteDetail';
import type { NoteKind } from '../bindings/NoteKind';
import type { NoteLink } from '../bindings/NoteLink';
import type { NoteSummary } from '../bindings/NoteSummary';

const HOUR = 3_600_000;
const REPO = 'github.com/useful/harness-desktop';
const ROOT = '/home/dev/projects/harness-desktop';

/** A note without its body, as the list reports it. */
export function noteSummaryOf({ body: _body, ...summary }: NoteContent): NoteSummary {
  return summary;
}

function ago(hours: number): string {
  return new Date(Date.now() - hours * HOUR).toISOString();
}

interface Seed {
  id: number;
  kind: NoteKind;
  title: string;
  body: string;
  scope?: 'repo' | 'global';
  place?: 'team' | 'local';
  tags?: string[];
  entities?: string[];
  source?: string;
  age: number;
  revision?: number;
  hits?: number;
  /** `other` for a note the server flagged as not written in English. */
  lang?: 'en' | 'other';
}

function note(seed: Seed): NoteContent {
  const scope = seed.scope ?? 'repo';

  return {
    id: seed.id,
    kind: seed.kind,
    title: seed.title,
    body: seed.body,
    scope,
    repo: scope === 'repo' ? REPO : null,
    place: seed.place ?? 'local',
    tags: seed.tags ?? [],
    entities: seed.entities ?? [],
    author: { source: seed.source ?? 'claude-code', root: scope === 'repo' ? ROOT : null },
    createdAt: ago(seed.age * 2),
    updatedAt: ago(seed.age),
    revision: seed.revision ?? 1,
    hits: seed.hits ?? 0,
    readOnly: false,
    hidden: false,
    lang: seed.lang ?? 'en',
  };
}

const CRAFTED: Seed[] = [
  {
    id: 3,
    kind: 'decision',
    title: 'Notes are read through the Memory CLI, never its database',
    body: 'The desktop app lists, reads and deletes notes by running `harness-memory notes ...`.\n\nWhy: the database may be a team Postgres or Turso, and only the server knows the routing between the team and this computer. Opening a file here would show a different set than the agents see.',
    tags: ['architecture', 'memory'],
    entities: ['harness-memory', 'notes list', 'cli_notes.rs'],
    place: 'team',
    age: 5,
    revision: 3,
    hits: 12,
  },
  {
    id: 4,
    kind: 'fix',
    title: 'Tauri window stayed blank after a hot reload on Wayland',
    body: 'Symptom: the window opened white and the console showed nothing.\n\nCause: WebKitGTK renders into a DMA buffer that the compositor drops when the GPU process restarts.\n\nFix: start with WEBKIT_DISABLE_DMABUF_RENDERER=1 in `bun run dev`.',
    tags: ['linux', 'webkit'],
    entities: ['WEBKIT_DISABLE_DMABUF_RENDERER', 'src-tauri/tauri.conf.json'],
    age: 28,
    hits: 7,
    lang: 'other',
  },
  {
    id: 5,
    kind: 'problem',
    title: 'The release build fails on macOS when the notarization queue is slow',
    body: 'notarytool waits 10 minutes by default and the job gives up before Apple answers on busy days.\n\nNo fix yet. A longer timeout in the workflow looks right, but it holds a runner for an hour.',
    tags: ['release', 'ci'],
    entities: ['.github/workflows/release.yml', 'notarytool'],
    place: 'team',
    source: 'omp',
    age: 50,
    hits: 2,
  },
  {
    id: 6,
    kind: 'outcome',
    title: 'Virtualising the session list cut the first paint from 900 ms to 80 ms',
    body: 'Measured with 1,800 sessions on the reference laptop. The cost was never the rows themselves but the 1,800 stat calls made while building them, now cached.',
    tags: ['performance'],
    entities: ['SessionList.svelte', 'sessions_list'],
    age: 90,
    revision: 2,
    hits: 5,
  },
  {
    id: 7,
    kind: 'optimisation',
    title: 'Parse transcripts in a worker thread, not on the command thread',
    body: 'Large transcripts blocked every other command for up to a second. Parsing moved to a blocking task with a small bounded channel, so the UI stays responsive while a long session loads.',
    tags: ['performance', 'rust'],
    entities: ['transcript.rs', 'spawn_blocking'],
    source: 'codex',
    age: 140,
    hits: 9,
    lang: 'other',
  },
  {
    id: 8,
    kind: 'location',
    title: 'Where the notes view keeps its state between visits',
    body: 'ui/src/lib/stores/view-state.svelte.ts holds the selected note, the search, the kind chip, the sort and the page. It outlives the view, and is cleared by `reset("notes")`.',
    tags: ['ui'],
    entities: ['ui/src/lib/stores/view-state.svelte.ts'],
    age: 200,
  },
  {
    id: 9,
    kind: 'note',
    title: 'Run the mock backend with ?notes=empty to see the empty state',
    body: 'Open the preview with `?notes=empty`, `?scripts=empty`, `?scripts=outdated` or `?scripts=failed` to see the other states of the Notes and Scripts views.',
    scope: 'global',
    tags: ['preview'],
    age: 300,
  },
  {
    id: 10,
    kind: 'decision',
    title: 'A hidden character in an agent-written text is shown, never rendered',
    body: 'Agents can put zero-width or bidirectional characters into a title or a body.‮ gnirts​ Showing them as escapes is the only way to read what was really written.',
    scope: 'global',
    place: 'team',
    tags: ['security'],
    entities: ['revealHidden'],
    source: 'opencode',
    age: 320,
    revision: 2,
    hits: 4,
  },
];

const VERBS = ['Cache', 'Retry', 'Debounce', 'Batch', 'Index', 'Stream', 'Throttle', 'Shard', 'Prefetch', 'Compact'];
const OBJECTS = ['the session list', 'transcript reads', 'git status', 'the diff view', 'extension scans', 'release lookups', 'config reads', 'the command palette'];
const KINDS: NoteKind[] = ['fix', 'problem', 'outcome', 'optimisation', 'location', 'decision', 'note'];

/** A long tail of notes, so the table has several pages to turn: kinds rotate, ages spread over weeks. */
function generated(count: number): Seed[] {
  return Array.from({ length: count }, (_, index) => {
    const kind = KINDS[index % KINDS.length];
    const title = `${VERBS[index % VERBS.length]} ${OBJECTS[(index * 3) % OBJECTS.length]}${index >= VERBS.length * 2 ? ` (${index + 1})` : ''}`;

    return {
      id: 100 + index,
      kind,
      title,
      body: `${title}.\n\nRecorded by an agent while working on ${REPO}. Check the linked notes before changing it again.`,
      scope: index % 5 === 0 ? 'global' : 'repo',
      place: index % 4 === 0 ? 'team' : 'local',
      tags: index % 3 === 0 ? ['performance'] : ['maintenance'],
      entities: index % 2 === 0 ? ['src-tauri/src/lib.rs'] : [],
      age: 400 + index * 9,
      hits: index % 6,
    } satisfies Seed;
  });
}

/** Notes agents might keep, of every kind, repository and global, in the team database and on this computer. Fresh on every call. */
export function sampleNotes(extra = 62): NoteContent[] {
  return [...CRAFTED, ...generated(extra)].map(note);
}

/** How the crafted notes relate: a decision and the fix that followed it, a problem and what solved it. */
const RELATIONS: { from: number; to: number; relation: string }[] = [
  { from: 4, to: 5, relation: 'related_to' },
  { from: 7, to: 6, relation: 'led_to' },
  { from: 3, to: 8, relation: 'documented_in' },
  { from: 10, to: 9, relation: 'see_also' },
  { from: 5, to: 4, relation: 'caused_by' },
];

/** The notes a note links to and those that link to it, as the server gives them. */
export function linksOf(id: number, notes: NoteContent[]): NoteLink[] {
  const byId = new Map(notes.map((candidate) => [candidate.id, candidate]));
  const link = (other: number, relation: string, direction: NoteLink['direction']): NoteLink[] => {
    const found = byId.get(other);

    return found ? [{ id: found.id, title: found.title, kind: found.kind, relation, direction }] : [];
  };

  return RELATIONS.flatMap((entry) => [...(entry.from === id ? link(entry.to, entry.relation, 'out') : []), ...(entry.to === id ? link(entry.from, entry.relation, 'in') : [])]);
}

export function detailOf(found: NoteContent, notes: NoteContent[]): NoteDetail {
  return { note: found, links: linksOf(found.id, notes) };
}
