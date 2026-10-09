import type { SessionMessagesPage } from '../bindings/SessionMessagesPage';
import type { SessionRef } from '../bindings/SessionRef';
import { IpcFailure } from '../transport';
import { fixtures } from './fixtures';
import type { MockState } from './state';
import type { Handlers } from './types';

type SessionCommands = 'sessions_list' | 'session_tree' | 'session_messages' | 'session_files_touched';

const MAX_PAGE = 200;

function requireSession(session: SessionRef): void {
  const known = fixtures.sessions.list[session.harness].some((summary) => summary.ref.id === session.id);
  if (!known) {
    throw IpcFailure.from({ code: 'notFound', message: 'That session no longer exists.' });
  }
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
  };
}

/** In-memory session store: the generated OMP and PI samples, served for any project the user opened. */
export function sessionHandlers(state: MockState): Pick<Handlers, SessionCommands> {
  return {
    sessions_list: ({ harness, project }) => {
      state.requireKnown(project);

      return structuredClone(fixtures.sessions.list[harness]);
    },

    session_tree: ({ session, project }) => {
      state.requireKnown(project);
      requireSession(session);

      return { ...structuredClone(fixtures.sessions.tree), ref: session };
    },

    session_messages: ({ session, project, cursor, limit }) => {
      state.requireKnown(project);
      requireSession(session);

      return pageOf(cursor, limit);
    },

    session_files_touched: ({ session, project }) => {
      state.requireKnown(project);
      requireSession(session);

      return [...fixtures.sessions.filesTouched];
    },
  };
}
