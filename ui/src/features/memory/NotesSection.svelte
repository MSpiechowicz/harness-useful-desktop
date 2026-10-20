<script lang="ts">
  import FileUp from '@lucide/svelte/icons/file-up';
  import NotebookText from '@lucide/svelte/icons/notebook-text';
  import { untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import DocNewButton from '$lib/components/docs/DocNewButton.svelte';
  import FilterMenu from '$lib/components/FilterMenu.svelte';
  import { ICON } from '$lib/components/icon';
  import type { Column } from '$lib/components/table';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { NoteDetail } from '$lib/ipc/bindings/NoteDetail';
  import type { NoteKind } from '$lib/ipc/bindings/NoteKind';
  import type { NoteSaved } from '$lib/ipc/bindings/NoteSaved';
  import type { NoteSummary } from '$lib/ipc/bindings/NoteSummary';
  import { ALL_REPOS } from '$lib/stores/memory-reach';
  import { deleteNote, hideNote, noteDetail, noteDetailKey, noteList, NOTES_PAGE_SIZE } from '$lib/stores/memoryNotes.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { formatDateTime, relativeTime } from '$lib/utils/format';
  import { clampPage, pageCount } from '$lib/utils/pagination';

  import SessionConfirm from '../sessions/SessionConfirm.svelte';
  import { noteEditor } from './editor-state.svelte';
  import HiddenFilter from './HiddenFilter.svelte';
  import MemoryFilterHint from './MemoryFilterHint.svelte';
  import MemoryRepoFilter from './MemoryRepoFilter.svelte';
  import MemorySearch from './MemorySearch.svelte';
  import MemoryTable from './MemoryTable.svelte';
  import { notesImport } from './notes-import-state.svelte';
  import { notesTeam } from './notes-team.svelte';
  import NoteEditor from './NoteEditor.svelte';
  import NoteKindBadge from './NoteKindBadge.svelte';
  import NotePanel from './NotePanel.svelte';
  import { KIND_LABELS, NOTE_KINDS, noteColumns, noteScopeText, totalCount } from './note-format';
  import { PLACE_LABELS, spelledOut } from './script-format';

  type KindChoice = NoteKind | 'all';

  interface Shown {
    note: NoteSummary | null;
    detail: NoteDetail | null;
  }

  let confirmer: SessionConfirm | undefined = $state();

  const path = $derived(viewState.memoryReach);
  const page = $derived(noteList.data);
  const availability = $derived(page && page.availability.kind !== 'available' ? page.availability : null);
  const rows = $derived(page?.notes ?? []);
  const total = $derived(page?.total ?? 0);
  const kinds = $derived(page?.kinds ?? []);
  // An older Harness Memory cannot combine the scope with the kind: it gets the kind only, and a hint that an update adds the scope.
  const combined = $derived(page?.combinedFilters ?? true);
  const searching = $derived(viewState.notesQuery.trim() !== '');
  const filtered = $derived(viewState.notesKind !== null || viewState.memoryReach !== ALL_REPOS || notesTeam.showHidden);
  // Team notes hidden on this computer: a switch to them shows once there are any, and stays while they are shown.
  const hiddenCount = $derived(page?.hiddenCount ?? 0);
  // Nothing at all is kept: no notes of any kind, and no search or filter that would explain it.
  const none = $derived(page !== null && availability === null && total === 0 && !searching && !filtered);

  const kindOptions = $derived([
    { value: 'all' as KindChoice, label: t('memory.filter.all'), count: totalCount(kinds) },
    ...NOTE_KINDS.map((kind) => ({ value: kind as KindChoice, label: t(KIND_LABELS[kind]), count: kinds.find((entry) => entry.kind === kind)?.count ?? 0 })),
  ]);

  // A search that finds nothing says so. So do filters that leave nothing: they stay chosen, with the way to clear them in the row above.
  const noMatch = $derived.by(() => {
    if (page === null || rows.length > 0 || total > 0) {
      return null;
    }

    return !searching && filtered ? t('memory.notes.noFilter') : t('memory.notes.noMatch');
  });

  function repoChosen(): void {
    viewState.notesPage = 0;
    viewState.noteId = null;
  }

  function showHidden(on: boolean): void {
    notesTeam.showHidden = on;
    viewState.notesPage = 0;
    viewState.noteId = null;
  }

  function chooseKind(kind: NoteKind | null): void {
    viewState.notesKind = kind;
    viewState.notesPage = 0;
  }

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

    const asHidden = notesTeam.showHidden;

    untrack(() => noteDetail.sync(id === null ? null : noteDetailKey(id, asHidden), where));
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

  // "New note" from the command palette opens the editor once the list is up, if there is a Harness Memory to save to.
  // Where there is none (missing, too old, not answering) the request is dropped, so the editor never opens later when it comes back.
  $effect(() => {
    if (!noteEditor.wantsNew) {
      return;
    }

    if (noteList.status === 'error' || availability !== null) {
      noteEditor.wantsNew = false;
    } else if (page !== null) {
      noteEditor.wantsNew = false;
      noteEditor.create();
    }
  });

  function edit(): void {
    if (detail) {
      noteEditor.edit(detail.note);
    }
  }

  function saved(result: NoteSaved, title: string): void {
    toasts.push({ title: t(result.outcome === 'created' ? 'memory.editor.note.toast.created' : 'memory.editor.note.toast.updated'), description: spelledOut(title), tone: 'success' });

    // A server that does not say which note it wrote leaves the list as read; the panel stays as it was.
    if (result.id !== null) {
      viewState.noteId = result.id;
    }
  }

  function hide(): void {
    const note = shown.note;

    if (note) {
      void hideNote(note.id, spelledOut(note.title), path, !note.hidden);
    }
  }

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

{#snippet filters()}
  <MemoryRepoFilter section="notes" onchange={repoChosen} />

  {#if hiddenCount > 0 || notesTeam.showHidden}
    <HiddenFilter count={hiddenCount} on={notesTeam.showHidden} onchange={showHidden} />
  {/if}

  <FilterMenu
    label={t('memory.filter.kind')}
    prefix={t('memory.filter.kind')}
    options={kindOptions}
    value={viewState.notesKind ?? 'all'}
    onchange={(next) => chooseKind(next === 'all' ? null : next)}
    onclear={() => chooseKind(null)}
    clearLabel={t('memory.filter.clearKind')}
    align="start"
  />
{/snippet}

{#snippet hint()}
  <MemoryFilterHint />
{/snippet}

{#snippet emptyAction()}
  <div class="flex flex-wrap items-center justify-center gap-2">
    <DocNewButton label={t('memory.editor.add.note')} onclick={() => noteEditor.create()} />

    <Button disabled={notesImport.picking} onclick={() => notesImport.begin()}>
      {#snippet leading()}<FileUp size={ICON.sm} aria-hidden="true" />{/snippet}
      {t('memory.import.empty')}
    </Button>
  </div>
{/snippet}

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
  {emptyAction}
  failedTitle={t('memory.notes.empty.failed.title')}
  loadingLabel={t('memory.notes.loading')}
  {noMatch}
  count={tn('memory.notes.count', total)}
  {filters}
  hint={combined ? undefined : hint}
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
      onedit={detail ? edit : undefined}
      ondelete={remove}
      onhide={hide}
      onclose={close}
    />
  {/snippet}
</MemoryTable>

<NoteEditor bind:open={noteEditor.open} note={noteEditor.target} links={detail && detail.note.id === noteEditor.target?.id ? detail.links : []} onsaved={saved} onopen={(id) => (viewState.noteId = id)} />
<SessionConfirm bind:this={confirmer} />
