import { afterEach, describe, expect, it } from 'vitest';

import type { KnowledgeImportEvent } from '$lib/ipc/bindings/KnowledgeImportEvent';
import type { KnowledgeIntake } from '$lib/ipc/bindings/KnowledgeIntake';
import type { KnowledgeMatch } from '$lib/ipc/bindings/KnowledgeMatch';
import type { KnowledgeSimilar } from '$lib/ipc/bindings/KnowledgeSimilar';
import { IpcFailure } from '$lib/ipc/transport';
import { type Landing, replacementsOf } from '$lib/utils/knowledge-match';

import { fakeTransport } from '../../../tests/helpers';
import { emptyRun, etaSeconds, fractionOf, KnowledgeImportStore, reduceImport } from './knowledgeImport.svelte';
import { toasts } from './toasts.svelte';

afterEach(() => {
  toasts.items = [];
});

const file = (path: string, status: 'added' | 'failed' | 'skipped' = 'added', reason: string | null = null): KnowledgeImportEvent => ({ type: 'file', path, status, reason, docId: null, chunks: null, embedding: null, warnings: [] });
const INTAKE: KnowledgeIntake = { token: 't1', files: [{ name: 'a.md', path: '/docs/a.md', bytes: 10 }], folders: [], skipped: [] };

describe('what the events of an import add up to', () => {
  it('lists the files in the order they came, with how far the import is', () => {
    const events: KnowledgeImportEvent[] = [{ type: 'plan', total: 3 }, file('/d/a.md'), { type: 'progress', done: 1, total: 3 }, file('/d/b.pdf', 'failed', 'a scan'), { type: 'progress', done: 2, total: 3 }];
    const run = events.reduce(reduceImport, emptyRun(1000));

    expect(run.rows.map((row) => [row.path, row.status, row.reason])).toEqual([
      ['/d/a.md', 'added', null],
      ['/d/b.pdf', 'failed', 'a scan'],
    ]);
    expect([run.total, run.done]).toEqual([3, 2]);
    expect(fractionOf(run)).toBeCloseTo(2 / 3);
  });

  it('keeps the counts, the cancel and the chunks waiting for a vector from the closing event', () => {
    const run = reduceImport(emptyRun(0), { type: 'done', counts: { added: 2, updated: 0, unchanged: 1, skipped: 0, failed: 1 }, cancelled: true, counter: 'tokenizer', embeddingPending: 12 });

    expect(run).toMatchObject({ cancelled: true, embeddingPending: 12, counts: { added: 2, failed: 1 } });
  });

  it('has no fraction before the files are counted, and no time left before the first file or after the last', () => {
    expect(fractionOf(emptyRun(0))).toBeNull();
    expect(etaSeconds({ ...emptyRun(0), total: 10, done: 0 }, 5000)).toBeNull();
    expect(etaSeconds({ ...emptyRun(0), total: 10, done: 10 }, 5000)).toBeNull();
    // Two files in 4 s: eight more take about 16 s.
    expect(etaSeconds({ ...emptyRun(1000), total: 10, done: 2 }, 5000)).toBe(16);
  });
});

describe('the import of what was picked or dropped', () => {
  it('stays on the first step when the native dialog is cancelled', async () => {
    const store = new KnowledgeImportStore(fakeTransport({ knowledge_pick_files: () => null }));

    await store.pick('files');

    expect(store.phase).toBe('idle');
    expect(store.picking).toBe(false);
  });

  it('stages a pick, sends its token with the links and folds the events into the run', async () => {
    const sent: unknown[] = [];
    const store = new KnowledgeImportStore(
      fakeTransport({
        knowledge_pick_files: () => INTAKE,
        knowledge_import: (args: { onEvent: (event: KnowledgeImportEvent) => void } & Record<string, unknown>) => {
          sent.push({ token: args.token, options: args.options });
          args.onEvent({ type: 'plan', total: 1 });
          args.onEvent(file('/docs/a.md'));
          args.onEvent({ type: 'progress', done: 1, total: 1 });
          args.onEvent({ type: 'done', counts: { added: 1, updated: 0, unchanged: 0, skipped: 0, failed: 0 }, cancelled: false, counter: 'tokenizer', embeddingPending: 3 });

          return { counts: { added: 1, updated: 0, unchanged: 0, skipped: 0, failed: 0 }, cancelled: false };
        },
      }),
    );

    await store.pick('files');
    store.links = [{ repo: 'git:github.com/acme/widget', globs: ['docs/**'] }];
    expect(store.phase).toBe('staged');

    await store.start();

    expect(sent).toEqual([{ token: 't1', options: { links: [{ repo: 'git:github.com/acme/widget', globs: ['docs/**'] }], replaces: null } }]);
    expect(store.phase).toBe('done');
    expect(store.run.rows).toHaveLength(1);
    expect(store.embeddingPending).toBe(3);
  });

  it('keeps going with the window closed and says how it ended in a toast', async () => {
    let finish: () => void = () => {};
    const store = new KnowledgeImportStore(
      fakeTransport({
        knowledge_pick_files: () => INTAKE,
        knowledge_import: (args: { onEvent: (event: KnowledgeImportEvent) => void }) =>
          new Promise((resolve) => {
            args.onEvent({ type: 'plan', total: 1 });
            finish = () => {
              args.onEvent({ type: 'done', counts: { added: 1, updated: 0, unchanged: 0, skipped: 0, failed: 1 }, cancelled: false, counter: 'tokenizer', embeddingPending: 0 });
              resolve({ counts: { added: 1, updated: 0, unchanged: 0, skipped: 0, failed: 1 }, cancelled: false });
            };
          }),
      }),
    );

    store.begin();
    await store.pick('files');
    const running = store.start();
    store.close();

    expect(store.running).toBe(true);
    expect(toasts.items).toHaveLength(0);

    finish();
    await running;

    expect(toasts.items.map((toast) => [toast.title, toast.description, toast.tone])).toEqual([['Import finished', '1 added or updated, 1 failed', 'warning']]);
  });

  it('asks the server to stop once and ends with the files before the cancel kept', async () => {
    let cancels = 0;
    let finish: () => void = () => {};
    const store = new KnowledgeImportStore(
      fakeTransport({
        knowledge_pick_files: () => INTAKE,
        knowledge_import_cancel: () => {
          cancels += 1;
          finish();

          return true;
        },
        knowledge_import: (args: { onEvent: (event: KnowledgeImportEvent) => void }) =>
          new Promise((resolve) => {
            finish = () => {
              args.onEvent({ type: 'done', counts: { added: 1, updated: 0, unchanged: 0, skipped: 0, failed: 0 }, cancelled: true, counter: 'tokenizer', embeddingPending: 0 });
              resolve({ counts: { added: 1, updated: 0, unchanged: 0, skipped: 0, failed: 0 }, cancelled: true });
            };
          }),
      }),
    );

    await store.pick('files');
    const running = store.start();

    await Promise.all([store.cancel(), store.cancel()]);
    await running;

    expect(cancels).toBe(1);
    expect(store.run.cancelled).toBe(true);
    expect(store.cancelling).toBe(false);
  });

  it('shows why an import that could not run at all stopped', async () => {
    const store = new KnowledgeImportStore(fakeTransport({ knowledge_pick_files: () => INTAKE, knowledge_import: () => Promise.reject(IpcFailure.from({ code: 'invalidInput', message: 'another import is running' })) }));

    store.begin();
    await store.pick('files');
    await store.start();

    expect(store.phase).toBe('done');
    expect(store.run.failure).toBe('another import is running');
  });

  it('opens the window on a drop, and ignores a drop while an import runs', async () => {
    const store = new KnowledgeImportStore(fakeTransport({ knowledge_drop_intake: () => INTAKE }));

    await store.takeDrop();

    expect([store.open, store.phase, store.intake?.token]).toEqual([true, 'staged', 't1']);

    store.phase = 'running';
    store.intake = null;
    await store.takeDrop();

    expect(store.intake).toBeNull();
  });

  it('embeds the waiting chunks and takes the notice away once they are', async () => {
    let asked: unknown = null;
    const store = new KnowledgeImportStore(fakeTransport({ knowledge_reindex: (args: unknown) => ((asked = args), { documents: 1, embedded: 5, failed: 0, cancelled: false, model: 'm' }) }));
    store.embeddingPending = 5;

    await store.embedNow();

    expect(asked).toEqual({ ids: null });
    expect(store.embeddingPending).toBe(0);
  });
});

describe('comparing the staged files with the stored documents', () => {
  const match = { id: 7, title: 'Guide', place: 'remote' as const, reason: 'sourceKey' as const, sharedChunks: 3, chunks: 3, updatedAt: '2026-10-01T00:00:00Z' };
  const answer = (found: boolean): KnowledgeSimilar => ({ items: [{ path: '/docs/a.md', match: found ? match : null, error: null }], offline: false });

  it('keeps the answer of the latest comparison when an older one comes back after it', async () => {
    const waiting: ((value: KnowledgeSimilar) => void)[] = [];
    const store = new KnowledgeImportStore(fakeTransport({ knowledge_pick_files: () => INTAKE, knowledge_similar: () => new Promise<KnowledgeSimilar>((resolve) => waiting.push(resolve)) }));
    await store.pick('files');

    const first = store.scan();
    store.links = [{ repo: 'git:github.com/a/b', globs: [] }];
    const second = store.scan();

    waiting[1](answer(true));
    await second;
    waiting[0](answer(false));
    await first;

    expect(store.rows?.[0].match?.title).toBe('Guide');
    expect(store.scanning).toBe(false);
  });

  it('compares once for the same files and the same first link', async () => {
    let asked = 0;
    const store = new KnowledgeImportStore(fakeTransport({ knowledge_pick_files: () => INTAKE, knowledge_similar: () => (asked += 1, answer(true)) }));
    await store.pick('files');

    await store.scan();
    await store.scan();
    store.links = [{ repo: 'git:github.com/a/b', globs: ['src/**'] }, ...store.links];
    await store.scan();
    await store.scan();

    expect(asked).toBe(2);
  });

  /** A store with the file `/docs/a.md` matched by `found`, the import going to `landing`, and the options the import sends. */
  async function staged(found: Partial<KnowledgeMatch>, landing: Landing) {
    const sent: { options: unknown } = { options: null };
    const store = new KnowledgeImportStore(
      fakeTransport({
        knowledge_pick_files: () => INTAKE,
        knowledge_similar: () => ({ items: [{ path: '/docs/a.md', match: { ...match, ...found }, error: null }], offline: false }),
        knowledge_import: (args: { options: unknown }) => ((sent.options = args.options), { counts: null, cancelled: false }),
      }),
    );
    store.landing = landing;
    await store.pick('files');
    await store.scan();

    return { store, sent };
  }

  it('updates a strong match in the place the import goes to, by default', async () => {
    for (const reason of ['sourceKey', 'sameContent', 'sharedChunks'] as const) {
      const { store, sent } = await staged({ reason, place: 'local', id: 'local:7' }, 'local');
      await store.start();

      expect(sent.options, reason).toMatchObject({ replacing: [{ path: '/docs/a.md', id: 'local:7' }] });
    }
  });

  it('updates a strong match when only one place is in use', async () => {
    const { store, sent } = await staged({}, 'one');
    await store.start();

    expect(sent.options).toMatchObject({ replacing: [{ path: '/docs/a.md', id: 7 }] });
  });

  it('adds a file as new by default when the match only has the same title', async () => {
    const { store, sent } = await staged({ reason: 'title' }, 'remote');
    await store.start();

    expect(sent.options).not.toHaveProperty('replacing');
  });

  it('adds a file as new by default when the match lies in another place than the import goes to', async () => {
    for (const landing of ['local', 'unknown'] as const) {
      const { store, sent } = await staged({ reason: 'sourceKey', place: 'remote' }, landing);
      await store.start();

      expect(sent.options, landing).not.toHaveProperty('replacing');
      expect(sent.options, landing).toMatchObject({ links: [] });
    }
  });

  it('sends a replacement the user switched to, for any reason, in the place the import goes to', async () => {
    const { store, sent } = await staged({ reason: 'title', place: 'local' }, 'local');
    store.setUpdate('/docs/a.md', true);
    await store.start();

    expect(sent.options).toMatchObject({ replacing: [{ path: '/docs/a.md', id: 7 }] });
  });

  it('never replaces a match kept in another place, whatever the user chose', async () => {
    const { store, sent } = await staged({ reason: 'sourceKey', place: 'local' }, 'remote');
    store.setUpdate('/docs/a.md', true);

    expect(replacementsOf(store.rows ?? [], store.choices, store.landing)).toEqual([]);

    await store.start();

    expect(sent.options).not.toHaveProperty('replacing');
  });

  it('sends nothing to replace for a file the user switched to be added', async () => {
    const { store, sent } = await staged({}, 'remote');
    store.setUpdate('/docs/a.md', false);
    await store.start();

    expect(sent.options).not.toHaveProperty('replacing');
  });

  it('sends a file set to update again after it was set to be added', async () => {
    const { store, sent } = await staged({}, 'remote');
    store.setUpdate('/docs/a.md', false);
    store.setUpdate('/docs/a.md', true);
    await store.start();

    expect(sent.options).toMatchObject({ replacing: [{ path: '/docs/a.md', id: 7 }] });
  });

  it('does not carry a choice over to another match of the same file', async () => {
    let found: KnowledgeMatch | null = match;
    const sent: { options: unknown } = { options: null };
    const store = new KnowledgeImportStore(
      fakeTransport({
        knowledge_pick_files: () => INTAKE,
        knowledge_similar: () => ({ items: [{ path: '/docs/a.md', match: found, error: null }], offline: false }),
        knowledge_import: (args: { options: unknown }) => ((sent.options = args.options), { counts: null, cancelled: false }),
      }),
    );
    store.landing = 'remote';
    await store.pick('files');
    await store.scan();
    store.setUpdate('/docs/a.md', false);

    // Another link finds another document, which a strong match replaces by default.
    found = { ...match, id: 8, title: 'Other guide' };
    store.links = [{ repo: 'git:github.com/a/b', globs: [] }];
    await store.scan();
    await store.start();

    expect(sent.options).toMatchObject({ replacing: [{ path: '/docs/a.md', id: 8 }] });
  });

  it('forgets a comparison when other files are staged', async () => {
    const store = new KnowledgeImportStore(fakeTransport({ knowledge_pick_files: () => INTAKE, knowledge_similar: () => answer(true) }));
    await store.pick('files');
    await store.scan();
    store.setUpdate('/docs/a.md', false);

    await store.pick('files');

    expect([store.similar, store.choices]).toEqual([null, {}]);
  });
});

