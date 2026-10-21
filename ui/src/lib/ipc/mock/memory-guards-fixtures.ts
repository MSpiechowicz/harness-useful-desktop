import type { HeldItem } from '../bindings/HeldItem';

const MINUTE = 60_000;
const ROOT = '/home/dev/projects/harness-desktop';

/** The chat key the sample held saves belong to until the first chat with a memory identity starts (see `MockMemoryGuards`). */
export const SAMPLE_CHAT_KEY = '00000000-0000-4000-8000-000000000000';

/** Held note save: a new note. */
export const HELD_NOTE_SAVE_ID = '6c1f3d2a-9b8e-4f7a-8c6d-5e4f3a2b1c0d';
/** Held note update: a change to note 3, with the note as saved now in `current`. */
export const HELD_NOTE_UPDATE_ID = '8e3f5a4c-1d0a-4b9c-8e7f-7a6b5c4d3e2f';
/** Held note update that only appends: text added after note 7's body (`append`), with the note as saved now in `current`. */
export const HELD_NOTE_APPEND_ID = '9a4b6c5d-2e1f-4c0b-9f8a-8b7c6d5e4f3a';
/** Held script save: a new script. */
export const HELD_SCRIPT_SAVE_ID = '7d2e4f3b-0c9f-4a8b-9d7e-6f5a4b3c2d1e';

/** The folder the mock "native dialog" of `dialog_pick_folder` picks ("Add repository…"). */
export const PICKED_FOLDER = '/home/demo/Projekty/website';

/** The revision (SHA-256 of the held content) of each sample held save. */
export const HELD_REVISIONS: Record<string, string> = {
  [HELD_NOTE_SAVE_ID]: '1f0e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0',
  [HELD_NOTE_UPDATE_ID]: '2e1f3c4d5a6b78899a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f708192a3b4c5',
  [HELD_NOTE_APPEND_ID]: '4c3b5a6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f6071829',
  [HELD_SCRIPT_SAVE_ID]: '3d2c4b5a6f7e8d9c0b1a29384756abcdef0123456789abcdef0123456789abcd',
};

/** A revision for the `n`th time the agent held a save again (`?held=changed`). */
export function reheldRevision(n: number): string {
  return n.toString(16).padStart(64, 'e');
}

function ago(minutes: number): string {
  return new Date(Date.now() - minutes * MINUTE).toISOString();
}

/** The saves an agent made while "Ask before saving" was on, newest first, as `harness-memory held list` reports them. */
export function sampleHeld(chatKey: string = SAMPLE_CHAT_KEY): HeldItem[] {
  const base = { chatKey, client: 'omp', projectPath: ROOT, repoRoot: ROOT };

  return [
    {
      ...base,
      heldId: HELD_SCRIPT_SAVE_ID,
      revision: HELD_REVISIONS[HELD_SCRIPT_SAVE_ID],
      createdAt: ago(1),
      kind: 'script',
      tool: 'script_save',
      arguments: {
        name: 'check-bindings',
        description: 'Regenerate the TS bindings and fail when they changed.',
        language: 'bash',
        body: ['#!/usr/bin/env bash', 'set -euo pipefail', 'cargo test -p harness_business export_bindings', 'git diff --exit-code ui/src/lib/ipc/bindings', ''].join('\n'),
        timeout_secs: 120,
        scope: 'repo',
        project_path: ROOT,
      },
      current: null,
    },
    {
      ...base,
      heldId: HELD_NOTE_UPDATE_ID,
      revision: HELD_REVISIONS[HELD_NOTE_UPDATE_ID],
      createdAt: ago(4),
      kind: 'note',
      tool: 'note_update',
      arguments: {
        id: 3,
        title: 'WebKitGTK caps the page at 60 fps',
        body: 'WebKitGTK falls back to a 60 fps timer when the monitor size is unknown. The page itself holds 60, so measure frame pacing in WebKitGTK, not in Chrome.',
        kind: 'problem',
        tags: ['webkit', 'perf'],
        project_path: ROOT,
      },
      current: {
        id: 3,
        title: 'WebKitGTK falls back to 60 fps',
        body: 'WebKitGTK falls back to a 60 fps timer here.',
        kind: 'problem',
        tags: ['webkit'],
        entities: ['WebKitGTK'],
        scope: 'repo',
      },
    },
    {
      ...base,
      heldId: HELD_NOTE_SAVE_ID,
      revision: HELD_REVISIONS[HELD_NOTE_SAVE_ID],
      createdAt: ago(9),
      kind: 'note',
      tool: 'note_save',
      arguments: {
        title: 'Bindings come from the export test',
        body: 'ts-rs writes ui/src/lib/ipc/bindings when `cargo test -p harness_business` runs. Never edit those files by hand.',
        kind: 'location',
        tags: ['ipc', 'ts-rs'],
        entities: ['ts-rs'],
        scope: 'repo',
        project_path: ROOT,
      },
      current: null,
    },
    {
      ...base,
      heldId: HELD_NOTE_APPEND_ID,
      revision: HELD_REVISIONS[HELD_NOTE_APPEND_ID],
      createdAt: ago(15),
      kind: 'note',
      tool: 'note_update',
      arguments: {
        id: 7,
        append: 'Run `bun run guard` before a commit. It checks file sizes too.',
        project_path: ROOT,
      },
      current: {
        id: 7,
        title: 'Source files stay under 500 lines',
        body: 'The size guard fails any file over 500 lines. Split it into modules.',
        kind: 'decision',
        tags: ['guard'],
        entities: [],
        scope: 'repo',
      },
    },
  ];
}
