<script lang="ts">
  import NotebookText from '@lucide/svelte/icons/notebook-text';
  import { untrack } from 'svelte';

  import type { Column } from '$lib/components/table';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { NoteDetail } from '$lib/ipc/bindings/NoteDetail';
  import type { NoteKind } from '$lib/ipc/bindings/NoteKind';
  import type { NoteSummary } from '$lib/ipc/bindings/NoteSummary';
  import { deleteNote, noteDetail, noteList, NOTES_PAGE_SIZE } from '$lib/stores/memoryNotes.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { formatDateTime, relativeTime } from '$lib/utils/format';
  import { clampPage, pageCount } from '$lib/utils/pagination';

  import SessionConfirm from '../sessions/SessionConfirm.svelte';
  import MemorySearch from './MemorySearch.svelte';
  import MemoryTable from './MemoryTable.svelte';
  import NoteKindBadge from './NoteKindBadge.svelte';
  import NotePanel from './NotePanel.svelte';
  import { KIND_GROUP_LABELS, KIND_LABELS, kindCount, NOTE_KINDS, noteColumns, noteScopeText, totalCount } from './note-format';
  import { PLACE_LABELS, spelledOut } from './script-format';

  type KindChip = NoteKind | 'all';

  interface Shown {
    note: NoteSummary | null;
    detail: NoteDetail | null;
  }

  let confirmer: SessionConfirm | undefined = $state();

  const path = $derived(project.active?.path ?? null);
  const page = $derived(noteList.data);
  const availability = $derived(page && page.availability.kind !== 'available' ? page.availability : null);
  const rows = $derived(page?.notes ?? []);
  const total = $derived(page?.total ?? 0);
  const kinds = $derived(page?.kinds ?? []);
  const searching = $derived(viewState.notesQuery.trim() !== '');
  // Nothing at all is kept: no notes of any kind, and no search or kind that would explain it.
  const none = $derived(page !== null && availability === null && totalCount(kinds) === 0 && !searching && viewState.notesKind === null);

  const chips = $derived([
    { value: 'all' as KindChip, label: t('memory.notes.chips.all'), count: totalCount(kinds) },
    ...NOTE_KINDS.map((kind) => ({ value: kind as KindChip, label: t(KIND_GROUP_LABELS[kind]), count: kindCount(kinds, kind) })),
  ]);

  // A search that finds nothing says so. So does a kind with no note: it stays chosen, and the table names it.
  const noMatch = $derived.by(() => {
    if (page === null || rows.length > 0 || total > 0) {
      return null;
    }

    const kind = chips.find((chip) => chip.value === (viewState.notesKind ?? 'all'));

    return searching || !kind || kind.value === 'all' ? t('memory.notes.noMatch') : t('memory.notes.noGroup', { filter: kind.label });
  });

  // The note whose panel is open. Its row is on this page only sometimes (a linked note, a page turned), so the read of it also gives the facts.
  const openId = $derived(viewState.noteId);
  const detail = $derived<NoteDetail | null>(noteDetail.data && noteDetail.data.note.id === openId ? noteDetail.data : null);
  const live = $derived<Shown>({ note: detail?.note ?? rows.find((row) => row.id === openId) ?? null, detail });

  // A panel that closes slides out showing the note it had, not an empty frame.
  let last = $state.raw<Shown>({ note: null, detail: null });
  const shown = $derived(openId === null ? last : live);

  $effect(() => {
    if (openId !== null && live.note) {
      last = live;
    }
  });

  $effect(() => {
    const id = openId;
    const where = path;

    untrack(() => noteDetail.sync(id === null ? null : String(id), where));
  });

  // A note that was deleted (here or by an agent) while its link or panel was open is no longer there to show.
  $effect(() => {
    if (noteDetail.status === 'error' && noteDetail.error?.code === 'notFound') {
      viewState.noteId = null;
    }
  });

  // A new search starts at the first page.
  let searched = viewState.notesQuery;

  $effect(() => {
    const query = viewState.notesQuery;

    if (query !== searched) {
      searched = query;
      viewState.notesPage = 0;
    }
  });

  // Deleting the last note of a page leaves the pager past the end.
  $effect(() => {
    if (page && total > 0 && viewState.notesPage >= pageCount(total, NOTES_PAGE_SIZE)) {
      viewState.notesPage = clampPage(viewState.notesPage, total, NOTES_PAGE_SIZE);
    }
  });

  function remove(): void {
    const note = shown.note;

    if (!note) {
      return;
    }

    void deleteNote(note.id, path, () =>
      confirmer?.ask({
        title: t('memory.notes.delete.title'),
        description: t('memory.notes.delete.body'),
        subject: {
          icon: NotebookText,
          name: spelledOut(note.title),
          details: [t(KIND_LABELS[note.kind]), noteScopeText(note), t(PLACE_LABELS[note.place]), relativeTime(note.updatedAt)].join(' · '),
          note: note.place === 'team' ? t('memory.notes.delete.teamNote') : undefined,
        },
        confirmLabel: t('memory.notes.delete.confirm'),
      }) ?? Promise.resolve(false),
    );
  }
</script>

{#if page && !availability && !none}
  <MemorySearch bind:value={viewState.notesQuery} label={t('memory.notes.search.label')} placeholder={t('memory.notes.search.placeholder')} />
{/if}

<MemoryTable
  loader={noteList}
  loaded={page !== null}
  {availability}
  {none}
  emptyIcon={NotebookText}
  emptyTitle={t('memory.notes.empty.none.title')}
  emptyBody={t('memory.notes.empty.none.body')}
  failedTitle={t('memory.notes.empty.failed.title')}
  loadingLabel={t('memory.notes.loading')}
  {noMatch}
  chips={{
    label: t('memory.notes.chips.label'),
    idBase: 'memory-notes-kind',
    options: chips,
    value: viewState.notesKind ?? 'all',
    onchange: (kind) => {
      viewState.notesKind = kind === 'all' ? null : kind;
      viewState.notesPage = 0;
    },
  }}
  count={tn('memory.notes.count', totalCount(kinds))}
  detailEmpty={t('memory.notes.detail.empty')}
  tableLabel={t('memory.notes.table.label')}
  tableId="memory-notes"
  columns={noteColumns()}
  {rows}
  sort={viewState.notesSort}
  onsort={(next) => {
    viewState.notesSort = next;
    viewState.notesPage = 0;
  }}
  page={viewState.notesPage}
  pageSize={NOTES_PAGE_SIZE}
  {total}
  onpage={(next) => (viewState.notesPage = next)}
  {openId}
  onopen={(id) => (viewState.noteId = id)}
>
  {#snippet cell(note: NoteSummary, column: Column)}
    {#if column.key === 'title'}
      <span class="text-fg">{spelledOut(note.title)}</span>
    {:else if column.key === 'kind'}
      <NoteKindBadge kind={note.kind} />
    {:else if column.key === 'scope'}
      <span class="text-fg-2" title={note.repo ?? undefined}>{noteScopeText(note)}</span>
    {:else if column.key === 'place'}
      <span class="text-fg-2">{t(PLACE_LABELS[note.place])}</span>
    {:else}
      <span class="tabular text-fg-3" title={formatDateTime(note.updatedAt)}>{relativeTime(note.updatedAt)}</span>
    {/if}
  {/snippet}

  {#snippet panel({ close, inline })}
    <NotePanel
      open={openId !== null}
      {inline}
      note={shown.note}
      detail={shown.detail}
      status={noteDetail.status}
      error={noteDetail.error?.message ?? null}
      onretry={() => void noteDetail.refresh()}
      onopennote={(id) => (viewState.noteId = id)}
      ondelete={remove}
      onclose={close}
    />
  {/snippet}
</MemoryTable>

<SessionConfirm bind:this={confirmer} />
