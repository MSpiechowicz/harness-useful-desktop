import type { NoteKind } from '$lib/ipc/bindings/NoteKind';
import type { NotesImportFormat } from '$lib/ipc/bindings/NotesImportFormat';

/** A file of three notes in each format, as shown on the first page of the import. The third note leaves out what is optional. */
export const IMPORT_EXAMPLES: Record<NotesImportFormat, string> = {
  csv: `title,body,kind,tags,scope
Benchmark in release mode,Debug builds are ten times slower.,decision,perf build,global
Blank window after hot reload,Restart the dev server.,problem,tauri,repo
Window settings,They live in tauri.conf.json.,location,,
`,
  json: `{
  "notes": [
    {
      "title": "Benchmark in release mode",
      "body": "Debug builds are ten times slower.",
      "kind": "decision",
      "tags": ["perf", "build"],
      "scope": "global"
    },
    {
      "title": "Blank window after hot reload",
      "body": "Restart the dev server.",
      "kind": "problem",
      "tags": ["tauri"],
      "scope": "repo"
    },
    {
      "title": "Window settings",
      "body": "They live in tauri.conf.json.",
      "kind": "location"
    }
  ]
}
`,
};

/** The kinds a file may name, in the order the format is described. A row that names none is a plain note. */
export const IMPORT_KINDS: readonly NoteKind[] = ['outcome', 'problem', 'fix', 'optimisation', 'location', 'decision', 'note'];
