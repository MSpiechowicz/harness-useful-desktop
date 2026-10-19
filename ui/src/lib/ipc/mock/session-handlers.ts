import type { SessionFolder } from '../bindings/SessionFolder';
import type { SessionMessagesPage } from '../bindings/SessionMessagesPage';
import type { SessionRef } from '../bindings/SessionRef';
import type { SessionRoutineTag } from '../bindings/SessionRoutineTag';
import type { SessionSummary } from '../bindings/SessionSummary';
import { IpcFailure } from '../transport';
import { fixtures } from './fixtures';
import sessionsListOpenCode from './fixtures/sessions/sessions_list.opencode.json';
import type { MockRoutines } from './routine-state';
import type { MockState } from './state';
import type { Handlers } from './types';

type SessionCommands =
  | 'sessions_list'
  | 'sessions_folder'
  | 'session_tree'
  | 'session_messages'
  | 'session_files_touched'
  | 'session_delete';

const MAX_PAGE = 200;

const keyOf = (session: SessionRef): string => `${session.harness}:${session.id}`;

/** The fixture sessions of `harness` that were not deleted. */
function listed(state: MockState, harness: SessionRef['harness']): SessionSummary[] {
  const all = harness === 'opencode' ? (sessionsListOpenCode as unknown as SessionSummary[]) : fixtures.sessions.list[harness];

  const kept = all.filter((summary) => !state.deletedSessions.has(keyOf(summary.ref)));

  return state.drifted() && kept[0] ? [startedInTerminal(kept[0]), ...kept] : kept;
}

/** The session a `?drift` preview shows up with once its time comes: a copy of the first one, as if started in a terminal. */
function startedInTerminal(template: SessionSummary): SessionSummary {
  return {
    ...structuredClone(template),
    ref: { ...template.ref, id: 'drifted-0000-terminal' },
    title: null,
    firstPrompt: 'Started in a terminal a moment ago',
    parentSession: null,
    updatedAt: new Date().toISOString(),
    createdAt: new Date().toISOString(),
  };
}

function requireSession(state: MockState, session: SessionRef): SessionSummary {
  const found = listed(state, session.harness).find((summary) => summary.ref.id === session.id);
  if (!found) {
    throw IpcFailure.from({ code: 'notFound', message: 'That session no longer exists.' });
  }

  return found;
}

const HOME = '/home/demo';

/** The harness's default session folder for `project` under the demo home (never configured by the project). */
function folderOf(harness: SessionRef['harness'], project: string): SessionFolder {
  // OpenCode keeps every project's sessions in one data folder, which the project cannot change.
  if (harness === 'opencode') {
    return { path: `${HOME}/.local/share/opencode`, configured: false };
  }

  const path =
    harness === 'pi'
      ? `${HOME}/.pi/agent/sessions/--${project.replace(/^\//, '').replaceAll('/', '-')}--`
      : `${HOME}/.omp/agent/sessions/-${project.replace(`${HOME}/`, '').replaceAll('/', '-')}`;

  return { path, configured: false };
}

/** What an OpenCode transcript holds: its sessions are a flat list of turns, with no tree to leave a branch summary behind. */
const OPENCODE_ROLES: readonly string[] = ['user', 'assistant', 'toolResult'];

/** The sample transcript as `harness` would read it. */
function transcriptOf(harness: SessionRef['harness']): { messages: SessionMessagesPage['messages']; stats: SessionMessagesPage['stats'] } {
  const { messages, stats } = fixtures.sessions.messages;
  if (harness !== 'opencode') {
    return { messages, stats };
  }

  const kept = messages.filter((message) => OPENCODE_ROLES.includes(message.role));

  return { messages: kept, stats: stats && { ...stats, messages: kept.length } };
}

/** Index-based cursor over the active path, like the real transcript reader. */
function pageOf(harness: SessionRef['harness'], cursor: string | null, limit: number): SessionMessagesPage {
  const { messages, stats } = transcriptOf(harness);
  const start = cursor === null ? 0 : Number(cursor);

  if (!Number.isInteger(start) || start < 0 || start > messages.length || limit < 1) {
    throw IpcFailure.from({ code: 'invalidInput', message: 'That page cursor is not valid.' });
  }

  const end = Math.min(start + Math.min(limit, MAX_PAGE), messages.length);

  return {
    messages: structuredClone(messages.slice(start, end)),
    nextCursor: end < messages.length ? String(end) : null,
    total: messages.length,
    stats: cursor === null ? structuredClone(stats) : null,
  };
}

/** The sessions of a project, with the ones a routine started tagged: one still going on (it asks the user something) and a finished one. */
function taggedInProject(sessions: SessionSummary[], tags: SessionRoutineTag[]): SessionSummary[] {
  const places = [1, 3];

  return sessions.map((summary, index) => {
    const tag = tags[places.indexOf(index)];

    return tag ? { ...summary, routine: tag } : summary;
  });
}

/** What the routines' own folder holds: copies of the first sessions, one for each finished run of a global routine, each in the folder of its routine. */
function inRoutinesFolder(sessions: SessionSummary[], tags: SessionRoutineTag[], folder: string): SessionSummary[] {
  return tags.slice(0, sessions.length).map((tag, index) => ({ ...sessions[index], cwd: `${folder}/${tag.routineId}`, routine: tag }));
}

/** In-memory session store: the generated OMP, PI and OpenCode samples, served for any project the user opened (and the routines' folder); deletes apply to every project. */
export function sessionHandlers(state: MockState, routines: Pick<MockRoutines, 'sessionTags' | 'workspace'>): Pick<Handlers, SessionCommands> {
  /** The folders a session command is for: the known projects, and where global routines keep their sessions. */
  function requireKnown(project: string | null): void {
    if (project === null || project !== routines.workspace()) {
      state.requireKnown(project);
    }
  }

  return {
    sessions_list: ({ harness, project }) => {
      requireKnown(project);
      const tags = routines.sessionTags();
      const sessions = structuredClone(listed(state, harness));

      return project === routines.workspace() ? inRoutinesFolder(sessions, tags.global, project) : taggedInProject(sessions, tags.project);
    },

    sessions_folder: ({ harness, project }) => {
      requireKnown(project);

      return folderOf(harness, project);
    },

    session_tree: ({ session, project }) => {
      requireKnown(project);
      requireSession(state, session);
      // OpenCode sessions are a plain list: no tree to branch.
      if (session.harness === 'opencode') {
        throw IpcFailure.from({ code: 'unsupported', message: 'OpenCode sessions have no tree.' });
      }

      return { ...structuredClone(fixtures.sessions.tree), ref: session };
    },

    session_messages: ({ session, project, cursor, limit }) => {
      requireKnown(project);
      requireSession(state, session);

      return pageOf(session.harness, cursor, limit);
    },

    session_files_touched: ({ session, project }) => {
      requireKnown(project);
      requireSession(state, session);

      return [...fixtures.sessions.filesTouched];
    },

    session_delete: ({ session, project, emptyCopyOnly }) => {
      requireKnown(project);
      const found = requireSession(state, session);

      if (emptyCopyOnly && !found.emptyCopy) {
        throw IpcFailure.from({ code: 'invalidInput', message: `invalid input: session ${session.id} is not an empty copy` });
      }

      state.deletedSessions.add(keyOf(session));
      return null;
    },
  };
}
