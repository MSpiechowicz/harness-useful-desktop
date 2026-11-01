import type { KnowledgeFileStatus } from '../bindings/KnowledgeFileStatus';
import type { KnowledgeImportCounts } from '../bindings/KnowledgeImportCounts';
import type { KnowledgeIntake } from '../bindings/KnowledgeIntake';
import { IpcFailure } from '../transport';
import { documentOf } from './knowledge-fixtures';
import type { MockKnowledge } from './knowledge-handlers';
import type { Handlers } from './types';

type ImportCommands = 'knowledge_pick_files' | 'knowledge_pick_folder' | 'knowledge_drop_intake' | 'knowledge_import' | 'knowledge_import_cancel';

/** Pause between two files of an import, so the progress can be seen. Tests do not wait. */
const FILE_MS = import.meta.env.MODE === 'test' ? 0 : 350;

const PICKED = ['onboarding-guide.md', 'api-reference.html', 'scan-of-contract.pdf', 'notes.docx', 'runbook.adoc'];

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
  const extension = name.split('.').pop() ?? '';

  return { md: 'markdown', html: 'html', adoc: 'asciidoc', rst: 'rst', txt: 'text' }[extension] ?? 'text';
}

function intakeOf(files: string[], folders: string[]): KnowledgeIntake {
  return {
    token: `intake-${files.length}-${folders.length}`,
    files: files.map((name) => ({ name, path: `/home/demo/Documents/${name}`, bytes: 24_000 })),
    folders: folders.map((name) => ({ name, path: `/home/demo/Documents/${name}`, bytes: null })),
    skipped: [],
  };
}

/** The pickers and the streamed import over the mock database. A second import while one runs is refused, a cancel ends it after the file in hand. */
export function knowledgeImportHandlers(store: MockKnowledge): Pick<Handlers, ImportCommands> {
  let running = false;
  let cancelled = false;

  return {
    knowledge_pick_files: () => intakeOf(PICKED, []),
    knowledge_pick_folder: () => intakeOf([], ['team-docs']),
    // Only asked after a drop, which the preview has as the window's own drag and drop: it drops the sample files.
    knowledge_drop_intake: () => intakeOf(PICKED, []),

    knowledge_import: async ({ options, onEvent }) => {
      if (running) {
        throw IpcFailure.from({ code: 'invalidInput', message: 'another import is running' });
      }

      running = true;
      cancelled = false;

      const counts: KnowledgeImportCounts = { added: 0, updated: 0, unchanged: 0, skipped: 0, failed: 0 };

      try {
        onEvent({ type: 'plan', total: PICKED.length });

        for (const [index, name] of PICKED.entries()) {
          await new Promise((resolve) => setTimeout(resolve, FILE_MS));

          if (cancelled) {
            break;
          }

          const { status, reason } = outcomeOf(name);
          const docId = status === 'added' ? store.newId(options?.place) : null;
          counts[status] += 1;

          if (docId !== null) {
            const doc = documentOf({ id: docId, title: name.replace(/\.[^.]+$/, ''), name, format: formatOf(name), kb: 24, chunks: 6, age: 0, links: options?.links ?? [] });

            store.documents.push(store.split ? { ...doc, place: options?.place ?? 'remote' } : doc);
            store.changed();
          }

          onEvent({ type: 'file', path: `/home/demo/Documents/${name}`, status, reason, docId, chunks: docId === null ? null : 6, embedding: docId === null ? null : 'done', warnings: [] });
          onEvent({ type: 'progress', done: index + 1, total: PICKED.length });
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
