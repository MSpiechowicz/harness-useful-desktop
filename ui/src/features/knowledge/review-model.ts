import { type MessageKey, t } from '$lib/i18n/i18n.svelte';
import type { KnowledgeDocStatus } from '$lib/ipc/bindings/KnowledgeDocStatus';
import type { KnowledgeCompare } from '$lib/ipc/bindings/KnowledgeCompare';
import type { KnowledgeDocument } from '$lib/ipc/bindings/KnowledgeDocument';
import type { KnowledgeHistoryEvent } from '$lib/ipc/bindings/KnowledgeHistoryEvent';
import { idKey, type KnowledgeId } from '$lib/ipc/knowledge-id';
import { relativeTime } from '$lib/utils/format';

/** The version a review acts on: a document that is not live, or the pending update of a live one. */
export interface Version {
  /** The row steps, comparisons and comments are asked of: the pending row for a live document that has one. */
  id: KnowledgeId;
  status: KnowledgeDocStatus;
  /** Given back to every step, so a version that changed meanwhile is not decided blindly. */
  revision: number;
  /** It replaces a live document (a new document has nothing to compare with). */
  update: boolean;
  /** Who last changed it: the person whose work it is. */
  author: string | null;
  /** Everybody the server's `selfApproval` and take-back rule counts as the version's writers (null from a server that does not say). */
  authors?: readonly string[] | null;
  updatedAt: string;
}

/** The `authors` a server printed on a document or a pending update, if it did. */
function authorsOf(source: object): readonly string[] | null {
  const { authors } = source as { authors?: readonly string[] | null };

  return authors && authors.length > 0 ? authors : null;
}

/** The version a document is reviewed as, or null for a live document that has no pending update. */
export function versionOf(doc: KnowledgeDocument): Version | null {
  if (doc.pending) {
    return { id: doc.pending.id, status: doc.pending.status, revision: doc.pending.revision, update: true, author: doc.pending.updatedBy?.name ?? null, authors: authorsOf(doc.pending), updatedAt: doc.pending.updatedAt };
  }

  if (doc.status === 'live') {
    return null;
  }

  return { id: doc.id, status: doc.status, revision: doc.revision, update: doc.pendingOf != null, author: doc.updatedBy?.name ?? doc.addedBy?.name ?? null, authors: authorsOf(doc), updatedAt: doc.updatedAt };
}

/** The live document a pending version replaces: the document itself when it has a pending update, the live row when it is the pending row, else none. */
export function liveIdOf(doc: KnowledgeDocument): KnowledgeId | null {
  if (doc.pending) {
    return doc.id;
  }

  return doc.pendingOf ?? null;
}

/** The revision a verdict is made on, and what stops it. */
export interface DecisionPin {
  /** The revision of the text the reviewer sees: given back to the server as the one the verdict is about. Null while there is none. */
  revision: number | null;
  /** Why a verdict (approve, ask for changes) must wait, or null when the diff on screen is the version's latest text. */
  reason: string | null;
}

/**
 * Pins a verdict to what is on screen. The server decides on the revision it is given, so the one to give is the revision of the diff the reviewer reads (`compare`), and only
 * while the version (as the document detail knows it) is still at that revision. A compare that is not on screen (null: loading, failed, refused), one of another row than the
 * version (the diff of the document shown before) or one of another revision waits.
 * A server that does not print the compare's row or revision cannot be pinned on it: the version's stands.
 */
export function decisionPin(compare: Pick<KnowledgeCompare, 'revision' | 'pending'> | null, version: Pick<Version, 'id' | 'revision'>): DecisionPin {
  if (compare === null || (compare.pending != null && idKey(compare.pending) !== idKey(version.id))) {
    return { revision: null, reason: t('knowledge.drafts.review.pinUnavailable') };
  }

  const revision = compare.revision ?? version.revision;

  return { revision, reason: revision === version.revision ? null : t('knowledge.drafts.review.pinChanged') };
}

/** The steps a review row offers. */
export type ReviewStep = 'approve' | 'requestChanges' | 'ask' | 'askAgain' | 'goLive' | 'takeBack' | 'replace';

export interface ReviewAction {
  step: ReviewStep;
  /** `primary` first, then `secondary`, then `dangerQuiet`: the order of the row. */
  variant: 'primary' | 'secondary' | 'dangerQuiet';
  label: string;
  /** Why the button is off, said in a grey line under the row and in its tooltip. */
  reason: string | null;
}

/** A comment a person left, shown under the headline in one text style. */
export interface ReviewQuote {
  heading: string;
  text: string;
  at: string | null;
}

export interface ReviewModel {
  headline: string;
  /** One grey line of context. */
  context: string | null;
  quote: ReviewQuote | null;
  actions: ReviewAction[];
  /** The author may write a note for the reviewers, which goes with a new file. */
  noteField: boolean;
}

export interface ReviewInput {
  version: Version;
  /** Team approval is on, off, or not known (no team database): only `true` asks teammates. */
  approval: boolean | null;
  /** The user wrote this version: true, false, or null while the user's name is not known. */
  mine: boolean | null;
  /** The user may not change what is kept in this place. */
  readOnly: boolean;
  /** The server said the user has no git name. */
  noName: boolean;
  /** Why a verdict waits for the diff on screen to be the version's latest text (see `decisionPin`), or null. */
  pin: string | null;
  /** The events of the version, newest first, when read. */
  events: readonly KnowledgeHistoryEvent[];
  /** The last request for changes the list knows of, for a version whose history is not read yet. */
  lastChange: KnowledgeDocument['lastChange'];
}

const newest = (events: readonly KnowledgeHistoryEvent[], kind: KnowledgeHistoryEvent['kind']): KnowledgeHistoryEvent | undefined => events.find((event) => event.kind === kind);

/** Whether the user is this person: true, false, or null while the user's name is not known. */
export type IsViewer = (name: string | null | undefined) => boolean | null;

/** Everybody who wrote what the version has now: the writers since the last time it went live (newest events come first), who last changed it and, while it waits, who asked. */
function writersOf(version: Pick<Version, 'author' | 'status'>, events: readonly KnowledgeHistoryEvent[]): (string | null)[] {
  const people: (string | null)[] = [version.author];

  for (const event of events) {
    if (event.kind === 'wentLive' || event.kind === 'approved') {
      break;
    }

    if ((event.kind === 'drafted' || event.kind === 'edited' || event.kind === 'updateDrafted') && event.by?.trim()) {
      people.push(event.by);
    }
  }

  if (version.status === 'waiting') {
    const asker = newest(events, 'asked')?.by;

    if (asker?.trim()) {
      people.push(asker);
    }
  }

  return people;
}

/**
 * Whether the user owns a version in the sense of the server's `selfApproval` rule: they wrote any of it since it last went live, or they are the one who asked for its review. The
 * owner may take the ask back and may not approve or ask for changes. The server's list of those people (`authors`) decides when it printed one, else the history and the last
 * writer stand in for it. Null while the user's name is not known.
 */
export function mineOf(version: Pick<Version, 'author' | 'authors' | 'status'>, events: readonly KnowledgeHistoryEvent[], is: IsViewer): boolean | null {
  const people = version.authors && version.authors.length > 0 ? version.authors : writersOf(version, events);
  const matches = people.map((person) => is(person));

  if (matches.includes(true)) {
    return true;
  }

  return matches.length === 0 || matches.includes(null) ? null : false;
}

function action(step: ReviewStep, variant: ReviewAction['variant'], label: string, reason: string | null): ReviewAction {
  return { step, variant, label, reason };
}

/** What stops a user from moving a version: a role that only reads, or no git name. */
function blockOf(input: ReviewInput): string | null {
  if (input.readOnly) {
    return t('knowledge.panel.readOnly');
  }

  return input.noName ? t('knowledge.drafts.review.noName') : null;
}

/** Why the author cannot take an ask back: only a role that reads is stopped (taking it back needs no git name, the author has one). */
function takeBackBlock(input: ReviewInput): string | null {
  return input.readOnly ? t('knowledge.panel.readOnly') : null;
}

/** The request for changes the version is waiting on: its comment, who wrote it and when. */
function requestOf(input: ReviewInput): { by: string | null; text: string | null; at: string | null } {
  const event = newest(input.events, 'changesRequested');

  if (event) {
    return { by: event.by ?? null, text: event.comment ?? null, at: event.at };
  }

  return { by: input.lastChange?.by ?? null, text: input.lastChange?.comment ?? null, at: input.lastChange?.at ?? null };
}

/**
 * What the Review section says and offers, from the side of the person reading it: a headline, one grey line, the comment that matters and one row of at most three buttons.
 * The author of a version never gets Approve, and gets "Take it back" while it waits. While the user's name is not known the version is treated as somebody else's when it waits (the server refuses a self approval and the
 * app learns the name from that) and as the user's own otherwise.
 */
export function reviewModel(input: ReviewInput): ReviewModel {
  const { version, approval, mine } = input;
  const own = mine !== false;
  /** The words for a new document, or for an update of a live one. */
  const pick = (document: MessageKey, update: MessageKey): MessageKey => (version.update ? update : document);
  const name = version.author ?? t('knowledge.drafts.someone');
  const block = blockOf(input);
  const keep = version.update ? ` ${t('knowledge.drafts.review.keepLive')}` : '';
  const asked = newest(input.events, 'asked');
  /** Who asked for the review: the one who replaced the file is not always the one who asked. */
  const asker = asked?.by ?? name;
  const when = relativeTime(asked?.at ?? version.updatedAt);
  const request = requestOf(input);
  const requested = request.by ?? t('knowledge.drafts.aTeammate');

  const quote: ReviewQuote | null =
    version.status === 'changesRequested' && request.text ? { heading: t('knowledge.drafts.review.requestOf', { name: requested }), text: request.text, at: request.at } : null;
  /** The author of a version that waits may take the ask back: it becomes a draft again. */
  const takeBack = version.status === 'waiting' && mine === true ? [action('takeBack', 'secondary', t('knowledge.drafts.review.takeBack'), takeBackBlock(input))] : [];
  const note: ReviewQuote | null =
    version.status === 'waiting' && asked?.comment
      ? { heading: t(mine === true ? 'knowledge.drafts.review.noteFromYou' : 'knowledge.drafts.review.noteFrom', { name: asker }), text: asked.comment, at: asked.at }
      : null;

  // Team approval is off, or there is no team database: whoever wrote it makes it live.
  if (approval !== true) {
    if (own) {
      return {
        headline: version.status === 'changesRequested' ? t('knowledge.drafts.review.requested', { name: requested }) : t('knowledge.drafts.review.readyToGoLive'),
        context: t(pick('knowledge.drafts.review.goLive.document', 'knowledge.drafts.review.goLive.update')),
        quote,
        actions: [action('goLive', 'primary', t('knowledge.drafts.review.goLive'), block), ...takeBack],
        noteField: false,
      };
    }

    return {
      headline: t(pick('knowledge.drafts.review.by.document', 'knowledge.drafts.review.by.update'), { name }),
      context: t('knowledge.drafts.review.onlyGoLive', { name }),
      quote,
      actions: [],
      noteField: false,
    };
  }

  if (version.status === 'draft') {
    if (own) {
      return {
        headline: t(pick('knowledge.drafts.review.draft.document', 'knowledge.drafts.review.draft.update')),
        context: t(pick('knowledge.drafts.review.draft.document.context', 'knowledge.drafts.review.draft.update.context')),
        quote: null,
        actions: [action('ask', 'primary', t('knowledge.drafts.review.ask'), block)],
        noteField: false,
      };
    }

    return {
      headline: t(pick('knowledge.drafts.review.by.document', 'knowledge.drafts.review.by.update'), { name }),
      context: t(pick('knowledge.drafts.review.draft.other.document', 'knowledge.drafts.review.draft.other.update'), { name }),
      quote: null,
      actions: [],
      noteField: false,
    };
  }

  if (version.status === 'changesRequested') {
    if (own) {
      return {
        headline: t('knowledge.drafts.review.requested', { name: requested }),
        context: `${t('knowledge.drafts.review.replaceThenAsk')}${keep}`,
        quote,
        actions: [
          action('askAgain', 'primary', t('knowledge.drafts.review.askAgain'), block),
          action('replace', 'secondary', t('knowledge.panel.replace'), input.readOnly ? t('knowledge.panel.readOnly') : null),
        ],
        noteField: true,
      };
    }

    return { headline: t('knowledge.drafts.review.waitingFor', { name }), context: `${t('knowledge.drafts.review.requestedBy', { name: requested })}${keep}`, quote, actions: [], noteField: false };
  }

  if (mine === true) {
    return {
      headline: t('knowledge.drafts.review.waitingTeammate'),
      context: t(pick('knowledge.drafts.review.waiting.document', 'knowledge.drafts.review.waiting.update'), { when }),
      quote: null,
      actions: takeBack,
      noteField: false,
    };
  }

  return {
    headline: t('knowledge.drafts.review.askedYou', { name: asker }),
    context: t(pick('knowledge.drafts.review.askedYou.document', 'knowledge.drafts.review.askedYou.update'), { when }),
    quote: note,
    actions: [
      action('approve', 'primary', t('knowledge.drafts.review.approve'), block ?? input.pin),
      action('requestChanges', 'secondary', t('knowledge.drafts.review.requestChanges'), block ?? input.pin),
    ],
    noteField: false,
  };
}
