import { waitFor } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';

import type { ReviewComment } from '$lib/ipc/bindings/ReviewComment';

import { PROJECT, storeWithMock } from '../../../tests/chat-helpers';
import { nav } from './nav.svelte';
import { MAX_COMMENTS, ReviewStore } from './review.svelte';

const COMMENT: ReviewComment = { path: 'src/lib.rs', side: 'new', startLine: 3, endLine: 5, body: 'Group these.', snippet: ['a', 'b', 'c'] };

// Drafts persist in the webview's storage; every test starts from none.
beforeEach(() => localStorage.clear());

function reviewStore() {
  const backend = storeWithMock();
  const review = new ReviewStore(backend.transport, backend.store, backend.notifications);

  return { review, ...backend };
}

describe('pending comments', () => {
  it('keeps comments per project until they are sent or discarded', () => {
    const { review } = reviewStore();

    review.add(PROJECT, COMMENT);
    review.add('/other/project', { ...COMMENT, body: 'Elsewhere.' });

    expect(review.commentsFor(PROJECT).map((comment) => comment.body)).toEqual(['Group these.']);
    expect(review.commentsFor('/other/project')).toHaveLength(1);

    review.discard(PROJECT);

    expect(review.commentsFor(PROJECT)).toEqual([]);
    expect(review.commentsFor('/other/project')).toHaveLength(1);
  });

  it('edits and removes a comment by id', () => {
    const { review } = reviewStore();
    review.add(PROJECT, COMMENT);
    review.add(PROJECT, { ...COMMENT, startLine: 9, endLine: 9, body: 'Second.' });
    const [first, second] = review.commentsFor(PROJECT);

    review.edit(PROJECT, first.id, 'Reworded.');
    review.remove(PROJECT, second.id);

    expect(review.commentsFor(PROJECT).map((comment) => comment.body)).toEqual(['Reworded.']);
  });

  it('refuses more comments than a review may hold', () => {
    const { review, notifications } = reviewStore();

    for (let index = 0; index < MAX_COMMENTS; index += 1) {
      expect(review.add(PROJECT, { ...COMMENT, startLine: index + 1, endLine: index + 1 })).toBe(true);
    }

    expect(review.add(PROJECT, COMMENT)).toBe(false);
    expect(review.commentsFor(PROJECT)).toHaveLength(MAX_COMMENTS);
    expect(notifications.items.at(-1)?.title).toContain('at most');
  });

  it('builds the request grouped by file in a stable order, without ids', () => {
    const { review } = reviewStore();
    review.add(PROJECT, { ...COMMENT, path: 'src/lib.rs', startLine: 20, endLine: 20 });
    review.add(PROJECT, { ...COMMENT, path: 'docs/guide.md', startLine: 1, endLine: 1 });
    review.add(PROJECT, { ...COMMENT, path: 'src/lib.rs', startLine: 3, endLine: 5 });
    review.setNote(PROJECT, '  Fix these, then run the tests.  ');

    const request = review.requestFor(PROJECT);

    expect(request.comments.map((comment) => [comment.path, comment.startLine])).toEqual([
      ['docs/guide.md', 1],
      ['src/lib.rs', 3],
      ['src/lib.rs', 20],
    ]);
    expect(request.comments.every((comment) => !('id' in comment))).toBe(true);
    expect(request.note).toBe('Fix these, then run the tests.');

    review.setNote(PROJECT, '   ');
    expect(review.requestFor(PROJECT).note).toBeNull();
  });
});

describe('composing and sending', () => {
  it('composes the prompt from the comments and the note', async () => {
    const { review } = reviewStore();
    review.add(PROJECT, COMMENT);
    review.setNote(PROJECT, 'Then run the tests.');

    const composed = await review.compose(PROJECT);

    expect(composed.commentCount).toBe(1);
    expect(composed.fileCount).toBe(1);
    expect(composed.prompt).toContain('Then run the tests.');
    expect(composed.prompt).toContain('src/lib.rs');
    expect(composed.prompt).toContain('Group these.');
  });

  it('sends to an idle chat as a prompt, clears the comments and opens the chat', async () => {
    const { review, store, of } = reviewStore();
    const chat = await store.create('omp', PROJECT);
    review.add(PROJECT, COMMENT);
    nav.go('changes');

    expect(await review.send(PROJECT, 'omp', chat?.key ?? '', 'the prompt')).toBe(true);

    expect(of('agent_send').filter(({ action }) => action.type === 'prompt')).toEqual([{ key: chat?.key, action: { type: 'prompt', message: 'the prompt', streamingBehavior: null } }]);
    expect(review.commentsFor(PROJECT)).toEqual([]);
    expect(store.activeKey).toBe(chat?.key);
    expect(nav.current).toBe('chat');
  });

  it('sends to a busy chat as a follow-up', async () => {
    const { review, store, of } = reviewStore();
    const chat = await store.create('omp', PROJECT);
    chat?.apply((state) => ({ ...state, phase: 'streaming' }));
    review.add(PROJECT, COMMENT);

    expect(await review.send(PROJECT, 'omp', chat?.key ?? '', 'the prompt')).toBe(true);

    expect(of('agent_send').map(({ action }) => action.type)).toContain('followUp');
    expect(of('agent_send').map(({ action }) => action.type)).not.toContain('prompt');
  });

  it('starts a new session for the review when asked to', async () => {
    const { review, store, of } = reviewStore();
    review.add(PROJECT, COMMENT);

    expect(await review.send(PROJECT, 'omp', 'new', 'the prompt')).toBe(true);

    expect(store.sessions).toHaveLength(1);
    expect(of('agent_start')[0].request).toMatchObject({ harness: 'omp', project: PROJECT, resume: null });
    await waitFor(() => expect(of('agent_send').some(({ action }) => action.type === 'prompt')).toBe(true));
    expect(review.commentsFor(PROJECT)).toEqual([]);
  });

  it('keeps the comments when the chat is gone or the agent refuses', async () => {
    const { review, notifications } = reviewStore();
    review.add(PROJECT, COMMENT);

    expect(await review.send(PROJECT, 'omp', 'missing-chat', 'the prompt')).toBe(false);

    expect(review.commentsFor(PROJECT)).toHaveLength(1);
    expect(notifications.items.at(-1)?.title).toBe('That chat is no longer open');
  });
});

describe('persistence', () => {
  it('keeps pending comments and the note across a restart, and forgets them once discarded', () => {
    const { review } = reviewStore();
    review.add(PROJECT, COMMENT);
    review.setNote(PROJECT, 'Then run the tests.');

    const reopened = reviewStore().review;
    expect(reopened.commentsFor(PROJECT)).toHaveLength(1);
    expect(reopened.commentsFor(PROJECT)[0].body).toBe(COMMENT.body);
    expect(reopened.noteFor(PROJECT)).toBe('Then run the tests.');

    reopened.add(PROJECT, { ...COMMENT, startLine: 9, endLine: 9 });
    expect(new Set(reopened.commentsFor(PROJECT).map((comment) => comment.id)).size).toBe(2);

    reopened.discard(PROJECT);
    expect(reviewStore().review.commentsFor(PROJECT)).toEqual([]);
  });

  it('ignores a corrupt saved record', () => {
    localStorage.setItem('harness:review-drafts', '{not json');

    expect(reviewStore().review.commentsFor(PROJECT)).toEqual([]);
  });
});
