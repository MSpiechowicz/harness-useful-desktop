import type { KnowledgeFileStatus } from '../bindings/KnowledgeFileStatus';
import type { KnowledgeImportCounts } from '../bindings/KnowledgeImportCounts';
import type { KnowledgeDocument } from '../bindings/KnowledgeDocument';
import type { KnowledgeIntake } from '../bindings/KnowledgeIntake';
import type { KnowledgeMatch } from '../bindings/KnowledgeMatch';
import type { KnowledgeSimilarEntry } from '../bindings/KnowledgeSimilarEntry';
import { idKey } from '../knowledge-id';
import { IpcFailure } from '../transport';
import { ADA } from './authors';
import { documentOf } from './knowledge-fixtures';
import type { MockKnowledge } from './knowledge-handlers';
import type { Handlers } from './types';

type ImportCommands = 'knowledge_pick_files' | 'knowledge_pick_folder' | 'knowledge_drop_intake' | 'knowledge_import' | 'knowledge_import_cancel' | 'knowledge_similar';

/** Pause between two files of an import, so the progress can be seen. Tests do not wait. */
const FILE_MS = import.meta.env.MODE === 'test' ? 0 : 350;

const PICKED = ['architecture.md', 'billing-openapi-v2.yaml', 'scan-of-contract.pdf', 'notes.docx', 'glossary.md'];

/** The files a picked folder holds. */
const FOLDER_FILES = ['team-docs/handbook.md', 'team-docs/release-process.adoc'];

const DIRECTORY = '/home/demo/Documents';

/** The extension of a file name. */
function extensionOf(name: string): string {
  return name.split('.').pop() ?? '';
}

/** The name of a file without its folder and extension (`team-docs/handbook.md` is `handbook`). */
function baseOf(name: string): string {
  return (name.split('/').pop() ?? name).replace(/\.[^.]+$/, '');
}

/** What the mock does with a picked file: a PDF without text fails, an unknown format is skipped. */
function outcomeOf(name: string): { status: KnowledgeFileStatus; reason: string | null } {
  if (name.endsWith('.pdf')) {
    return { status: 'failed', reason: 'the PDF has no text layer (a scan)' };
  }

  if (name.endsWith('.docx')) {
    return { status: 'skipped', reason: 'not documentation' };
  }

  return { status: 'added', reason: null };
}

function formatOf(name: string): string {
  return { md: 'markdown', html: 'html', adoc: 'asciidoc', rst: 'rst', txt: 'text', yaml: 'openapi' }[extensionOf(name)] ?? 'text';
}

/** Browser-preview switch `?kpick=one`: a drop answers one file instead of five, which is what replacing a document by a drop needs. */
function pickedFiles(search: string = globalThis.location?.search ?? ''): string[] {
  return new URLSearchParams(search).get('kpick') === 'one' ? PICKED.slice(0, 1) : PICKED;
}

function intakeOf(files: string[], folders: string[]): KnowledgeIntake {
  return {
    token: `intake-${files.length}-${folders.length}`,
    files: files.map((name) => ({ name, path: `${DIRECTORY}/${name}`, bytes: 24_000 })),
    folders: folders.map((name) => ({ name, path: `${DIRECTORY}/${name}`, bytes: null })),
    skipped: [],
  };
}

/**
 * What the comparison of the mock finds for a file: the document imported from the same path (`sourceKey`), one whose file is a `-copy` of it (`sameContent`),
 * a `-v2` of it (`sharedChunks`, most of the chunks), or one with the same title in the scope of the import's first link (`title`).
 */
function matchOf(store: MockKnowledge, name: string, repo: string | null): KnowledgeMatch | null {
  const base = baseOf(name);
  const found = (doc: KnowledgeDocument, reason: KnowledgeMatch['reason'], chunks: number, shared: number): KnowledgeMatch => ({
    id: doc.id,
    title: doc.title,
    place: doc.place ?? 'remote',
    reason,
    sharedChunks: shared,
    chunks,
    addedBy: doc.addedBy ?? null,
    updatedBy: doc.updatedBy ?? null,
    updatedAt: doc.updatedAt,
  });
  const scoped = (doc: KnowledgeDocument) => (repo === null ? doc.links.length === 0 : doc.links.some((link) => link.repo === repo));
  const documents = store.reachable();

  const same = documents.find((doc) => doc.sourceKey === `docs/${name}`);

  if (same) {
    return found(same, 'sourceKey', same.chunkCount, same.chunkCount);
  }

  const copy = documents.find((doc) => baseOf(doc.sourceName) === base.replace(/-copy$/, '') && base.endsWith('-copy'));

  if (copy) {
    return found(copy, 'sameContent', copy.chunkCount, copy.chunkCount);
  }

  const edited = documents.find((doc) => base.endsWith('-v2') && baseOf(doc.sourceName) === base.replace(/-v2$/, ''));

  if (edited) {
    const chunks = edited.chunkCount + 2;

    return found(edited, 'sharedChunks', chunks, Math.ceil(chunks * 0.7));
  }

  const titled = documents.find((doc) => scoped(doc) && doc.title.trim().toLowerCase() === base.replace(/[-_]+/g, ' ').toLowerCase());

  return titled ? found(titled, 'title', 4, 0) : null;
}

/** The pickers, the comparison and the streamed import over the mock database. A second import while one runs is refused, a cancel ends it after the file in hand. */
export function knowledgeImportHandlers(store: MockKnowledge): Pick<Handlers, ImportCommands> {
  let running = false;
  let cancelled = false;
  return {
    // The dialog of a replace takes one file, which the preview's single choice answers.
    knowledge_pick_files: (args) => intakeOf(args?.multiple === false ? PICKED.slice(0, 1) : pickedFiles(), []),
    knowledge_pick_folder: () => intakeOf([], ['team-docs']),
    // Only asked after a drop, which the preview has as the window's own drag and drop: it drops the sample files.
    knowledge_drop_intake: () => intakeOf(pickedFiles(), []),

    knowledge_similar: ({ token, links }) => {
      // The preview's tokens say what they hold (`intake-5-0` is the five picked files), any other token is gone.
      const [, files, folders] = /^intake-(\d+)-(\d+)$/.exec(token) ?? [];

      if (files === undefined) {
        throw IpcFailure.from({ code: 'notFound', message: 'that selection of files was not found' });
      }

      const intake = intakeOf(PICKED.slice(0, Number(files)), ['team-docs'].slice(0, Number(folders)));

      const repo = links?.[0]?.repo ?? null;
      // A folder gives its files, as the import walks it.
      const names = [...intake.files.map((file) => file.name), ...intake.folders.flatMap(() => FOLDER_FILES)];
      const items: KnowledgeSimilarEntry[] = names.map((name) => {
        const path = `${DIRECTORY}/${name}`;
        const { status, reason } = outcomeOf(name);

        return status === 'added' ? { path, match: matchOf(store, name, repo), error: null } : { path, match: null, error: reason };
      });

      return { items, offline: store.offline };
    },

    knowledge_import: async ({ token, options, onEvent }) => {
      if (running) {
        throw IpcFailure.from({ code: 'invalidInput', message: 'another import is running' });
      }

      running = true;
      cancelled = false;

      const counts: KnowledgeImportCounts = { added: 0, updated: 0, unchanged: 0, skipped: 0, failed: 0 };
      // The preview's tokens say how many of the picked files they hold (`intake-1-0` is the first one alone). A folder, or any other token, imports the sample files.
      const chosen = PICKED.slice(0, Number(/^intake-(\d+)-/.exec(token)?.[1]) || PICKED.length);

      try {
        // Like the sidecar: every replacement is checked before the first file, and an import that names another place than
        // a document to replace lives in is refused as a whole (a replacement files the new version in its document's place).
        const replacedIds = [...(options?.replacing ?? []).map((candidate) => candidate.id), ...(options?.replaces == null ? [] : [options.replaces])];
        const elsewhere = replacedIds.some((id) => {
          const doc = store.documents.find((candidate) => idKey(candidate.id) === idKey(id));

          return doc !== undefined && store.split && options?.place != null && (doc.place ?? 'remote') !== options.place;
        });

        if (elsewhere) {
          throw IpcFailure.from({ code: 'invalidInput', message: 'the document to replace lives in another place than the import files its documents in' });
        }

        onEvent({ type: 'plan', total: chosen.length });

        for (const [index, name] of chosen.entries()) {
          await new Promise((resolve) => setTimeout(resolve, FILE_MS));

          if (cancelled) {
            break;
          }

          const outcome = outcomeOf(name);
          const path = `${DIRECTORY}/${name}`;
          // A file named in `replacing`, or the one file of an import that `replaces` a document, takes the place of that document: it is updated in place and keeps who added it.
          const replacement = options?.replacing?.find((candidate) => candidate.path === path) ?? (options?.replaces == null ? undefined : { id: options.replaces });
          const replaced = replacement && outcome.status === 'added' ? store.documents.find((doc) => idKey(doc.id) === idKey(replacement.id)) : undefined;

          const status = replaced ? 'updated' : outcome.status;
          const { reason } = outcome;
          const docId = replaced?.id ?? (status === 'added' ? store.newId(options?.place) : null);
          counts[status] += 1;

          if (replaced) {
            // A new object, as a server answers with what it holds now: a reader that kept the old one sees the change.
            store.documents[store.documents.indexOf(replaced)] = { ...replaced, updatedAt: new Date().toISOString(), updatedBy: ADA };
            store.changed();
          } else if (docId !== null) {
            const doc = documentOf({ id: docId, title: name.replace(/\.[^.]+$/, ''), name, format: formatOf(name), kb: 24, chunks: 6, age: 0, links: options?.links ?? [], addedBy: ADA });

            store.documents.push(store.split ? { ...doc, place: options?.place ?? 'remote' } : doc);
            store.changed();
          }

          onEvent({ type: 'file', path, status, reason, docId, chunks: docId === null ? null : 6, embedding: docId === null ? null : 'done', warnings: [] });
          onEvent({ type: 'progress', done: index + 1, total: chosen.length });
        }

        onEvent({ type: 'done', counts, cancelled, counter: 'tokenizer', embeddingPending: 0 });

        return { counts, cancelled };
      } finally {
        running = false;
      }
    },

    knowledge_import_cancel: () => {
      cancelled = running;

      return running;
    },
  };
}
