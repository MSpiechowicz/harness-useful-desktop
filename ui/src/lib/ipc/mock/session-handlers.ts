import type { SessionMessagesPage } from '../bindings/SessionMessagesPage';
import type { SessionRef } from '../bindings/SessionRef';
import type { SessionSummary } from '../bindings/SessionSummary';
import { IpcFailure } from '../transport';
import { fixtures } from './fixtures';
import type { MockState } from './state';
import type { Handlers } from './types';

type SessionCommands = 'sessions_list' | 'session_tree' | 'session_messages' | 'session_files_touched' | 'session_delete';

const MAX_PAGE = 200;

const keyOf = (session: SessionRef): string => `${session.harness}:${session.id}`;

/** The fixture sessions of `harness` that were not deleted. */
function listed(state: MockState, harness: SessionRef['harness']): SessionSummary[] {
  return fixtures.sessions.list[harness].filter((summary) => !state.deletedSessions.has(keyOf(summary.ref)));
}

function requireSession(state: MockState, session: SessionRef): SessionSummary {
  const found = listed(state, session.harness).find((summary) => summary.ref.id === session.id);
  if (!found) {
    throw IpcFailure.from({ code: 'notFound', message: 'That session no longer exists.' });
  }

  return found;
}

/** Index-based cursor over the active path, like the real transcript reader. */
function pageOf(cursor: string | null, limit: number): SessionMessagesPage {
  const { messages } = fixtures.sessions.messages;
  const start = cursor === null ? 0 : Number(cursor);

  if (!Number.isInteger(start) || start < 0 || start > messages.length || limit < 1) {
    throw IpcFailure.from({ code: 'invalidInput', message: 'That page cursor is not valid.' });
  }

  const end = Math.min(start + Math.min(limit, MAX_PAGE), messages.length);

  return {
    messages: structuredClone(messages.slice(start, end)),
    nextCursor: end < messages.length ? String(end) : null,
    total: messages.length,
    stats: cursor === null ? structuredClone(fixtures.sessions.messages.stats) : null,
  };
}

/** In-memory session store: the generated OMP and PI samples, served for any project the user opened; deletes apply to every project. */
export function sessionHandlers(state: MockState): Pick<Handlers, SessionCommands> {
  return {
    sessions_list: ({ harness, project }) => {
      state.requireKnown(project);

      return structuredClone(listed(state, harness));
    },

    session_tree: ({ session, project }) => {
      state.requireKnown(project);
      requireSession(state, session);

      return { ...structuredClone(fixtures.sessions.tree), ref: session };
    },

    session_messages: ({ session, project, cursor, limit }) => {
      state.requireKnown(project);
      requireSession(state, session);

      return pageOf(cursor, limit);
    },

    session_files_touched: ({ session, project }) => {
      state.requireKnown(project);
      requireSession(state, session);

      return [...fixtures.sessions.filesTouched];
    },

    session_delete: ({ session, project, emptyCopyOnly }) => {
      state.requireKnown(project);
      const found = requireSession(state, session);

      if (emptyCopyOnly && !found.emptyCopy) {
        throw IpcFailure.from({ code: 'invalidInput', message: `invalid input: session ${session.id} is not an empty copy` });
      }

      state.deletedSessions.add(keyOf(session));
      return null;
    },
  };
}
