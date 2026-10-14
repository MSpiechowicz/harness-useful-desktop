<script lang="ts">
  import Pencil from '@lucide/svelte/icons/pencil';
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import X from '@lucide/svelte/icons/x';

  import Button from '$lib/components/Button.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import DiffViewComposer from '$lib/components/DiffViewComposer.svelte';
  import IconButton from '$lib/components/IconButton.svelte';
  import Overlay from '$lib/components/Overlay.svelte';
  import SearchField from '$lib/components/SearchField.svelte';
  import { commentHeading } from '$lib/git/comment-range';
  import { cutPieces, matchRanges, normalizeQuery, type Piece } from '$lib/git/search';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { byteLength, MAX_TEXT_BYTES, review, type DraftComment } from '$lib/stores/review.svelte';
  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  interface Props {
    open: boolean;
    project: string;
    /** The review is ready to send: the dialog closes and the send step opens. */
    onpreview: () => void;
  }

  let { open = $bindable(), project, onpreview }: Props = $props();

  let query = $state('');
  let chosen = $state<string | null>(null);
  let editing = $state<number | null>(null);
  let deleting = $state<DraftComment | null>(null);
  let confirmDelete = $state(false);
  let confirmDiscard = $state(false);

  const comments = $derived(review.commentsFor(project));
  const note = $derived(review.noteFor(project));
  const noteTooLong = $derived(byteLength(note) > MAX_TEXT_BYTES);
  const needle = $derived(normalizeQuery(query));

  /** What the search keeps: a comment whose text matches, or every comment of a file whose path does. */
  const kept = $derived(
    needle === null ? comments : comments.filter((comment) => comment.path.toLowerCase().includes(needle) || comment.body.toLowerCase().includes(needle)),
  );

  const files = $derived.by(() => {
    const byPath = new Map<string, DraftComment[]>();
    for (const comment of kept) {
      byPath.set(comment.path, [...(byPath.get(comment.path) ?? []), comment]);
    }

    return [...byPath.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([path, list]) => ({ path, list }));
  });

  /** Searching lists every matching file's comments; otherwise only the chosen file's. */
  const shown = $derived(needle === null ? files.filter((file) => file.path === (chosen ?? files[0]?.path)) : files);
  const current = $derived(needle === null ? (chosen ?? files[0]?.path ?? null) : null);

  const titleId = uid('review-dialog');
  const summary = $derived(tn('changes.review.found', comments.length, { files: tn('changes.review.files', new Set(comments.map((comment) => comment.path)).size) }));

  // Deleting the last comment leaves nothing to review.
  $effect(() => {
    if (open && comments.length === 0) {
      open = false;
    }
  });

  // A new search starts from the top.
  $effect(() => {
    void query;
    editing = null;
  });

  function split(path: string): { dir: string; name: string } {
    const cut = path.lastIndexOf('/');

    return cut === -1 ? { dir: '', name: path } : { dir: path.slice(0, cut + 1), name: path.slice(cut + 1) };
  }

  /** The path in two pieces (folder, file name), each cut at the edges of the words the search found. */
  function pathPieces(path: string): { dir: Piece[]; name: Piece[] } {
    const parts = split(path);
    const spans = needle === null ? [] : matchRanges(path, needle);

    return { dir: cutPieces(parts.dir, 0, spans), name: cutPieces(parts.name, parts.dir.length, spans) };
  }

  function textPieces(text: string): Piece[] {
    return cutPieces(text, 0, needle === null ? [] : matchRanges(text, needle));
  }

  /** "L29–30": the lines without the side, which the dialog groups by file anyway. */
  function lines(comment: DraftComment): string {
    return comment.startLine === comment.endLine ? `L${comment.startLine}` : `L${comment.startLine}–${comment.endLine}`;
  }

  function jump(comment: DraftComment): void {
    git.select(comment.path);
    open = false;
  }

  function ask(comment: DraftComment): void {
    deleting = comment;
    confirmDelete = true;
  }

  function discard(): void {
    review.discard(project);
    open = false;
  }

  function onSearchKey(event: KeyboardEvent): void {
    if (event.key === 'Escape' && query !== '') {
      event.preventDefault();
      event.stopPropagation();
      query = '';
    }
  }
</script>

{#snippet marked(pieces: Piece[])}{#each pieces as piece, index (index)}{#if piece.hit}<mark class="rounded-xs bg-warning/30 text-inherit">{piece.text}</mark>{:else}{piece.text}{/if}{/each}{/snippet}

<Overlay {open} labelledby={titleId} onclose={() => (open = false)}>
  <div class="flex max-h-[88vh] w-[min(62rem,calc(100vw-2rem))] flex-col overflow-hidden rounded-xl border border-line-strong bg-glass shadow-pop backdrop-blur-xl">
    <header class="flex items-center gap-3 border-b border-line px-4 py-3">
      <h2 id={titleId} class="text-xl text-fg">{t('changes.review.section')}</h2>
      <p class="text-sm text-fg-3">{summary}</p>
      <span class="flex-1"></span>
      <SearchField
        bind:value={query}
        label={t('changes.review.search')}
        placeholder={t('changes.review.search')}
        shortcut="mod+f"
        status={needle !== null && kept.length > 0 ? tn('changes.review.found', kept.length, { files: tn('changes.review.files', files.length) }) : undefined}
        class="w-[360px]"
        onkeydown={onSearchKey}
      />
      <IconButton icon={X} label={t('components.dialog.close')} onclick={() => (open = false)} />
    </header>

    {#if needle !== null && kept.length === 0}
      <!-- One block in the middle of the whole area, not a message in one column and a button lost in the other. -->
      <div class="grid h-[min(50vh,400px)] place-items-center text-center" role="status">
        <div class="flex flex-col items-center gap-3">
          <MessageSquare size={24} class="text-fg-3" aria-hidden="true" />
          <p class="text-base text-fg-2">{t('changes.review.nothing', { query: query.trim() })}</p>
          <Button onclick={() => (query = '')}>{t('changes.review.clear')}</Button>
        </div>
      </div>
    {:else}
      <div class="flex h-[min(50vh,400px)] min-h-0">
        <ul class="flex w-[270px] shrink-0 flex-col gap-1 overflow-y-auto border-r border-line p-2.5" aria-label={t('changes.review.filesList')}>
          {#each files as file (file.path)}
            {@const pieces = pathPieces(file.path)}
            <li>
              <button
                type="button"
                aria-current={current === file.path ? 'true' : undefined}
                class={cx(
                  'flex min-h-[36px] w-full items-center gap-2 rounded-md border px-2.5 py-1 text-left text-sm transition-colors duration-[var(--dur-fast)]',
                  current === file.path ? 'border-accent/30 bg-accent-wash' : 'border-transparent hover:bg-hover',
                )}
                title={file.path}
                onclick={() => (chosen = file.path)}
              >
                <span class="min-w-0 flex-1 truncate text-fg"><span class="text-fg-3">{@render marked(pieces.dir)}</span>{@render marked(pieces.name)}</span>
                <span class="tabular shrink-0 text-xs text-fg-3">{file.list.length}</span>
              </button>
            </li>
          {/each}
        </ul>

        <div class="flex min-w-0 flex-1 flex-col p-2.5">
          <div class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto">
            {#each shown as file (file.path)}
              <section class="flex flex-col gap-1">
                <h3 class="flex items-center gap-2 px-2 text-[11px] font-medium tracking-wide text-fg-3 uppercase">
                  <span class="min-w-0 flex-1 truncate font-mono tracking-normal text-accent-text normal-case">{@render marked(pathPieces(file.path).dir)}{@render marked(pathPieces(file.path).name)}</span>
                  <span class="tabular">{tn('changes.review.comments', file.list.length)}</span>
                </h3>

                <ul class="flex flex-col gap-1" aria-label={t('changes.review.pending')}>
                  {#each file.list as comment (comment.id)}
                    <li>
                      {#if editing === comment.id}
                        <DiffViewComposer
                          heading={commentHeading(comment)}
                          initial={comment.body}
                          saveLabel={t('components.diff.save')}
                          maxBytes={MAX_TEXT_BYTES}
                          onsave={(body) => {
                            review.edit(project, comment.id, body);
                            editing = null;
                          }}
                          oncancel={() => (editing = null)}
                        />
                      {:else}
                        <div class="flex items-center gap-2 rounded-md py-1 pr-1 pl-2 transition-colors duration-[var(--dur-fast)] hover:bg-hover">
                          <button type="button" class="flex min-w-0 flex-1 items-baseline gap-4 py-1 text-left" title={comment.body} onclick={() => jump(comment)}>
                            <span class="tabular w-14 shrink-0 font-mono text-xs text-fg-3">{lines(comment)}</span>
                            <span class="line-clamp-2 min-w-0 flex-1 text-sm break-words text-fg">{@render marked(textPieces(comment.body))}</span>
                          </button>
                          <IconButton icon={Pencil} label={t('components.diff.edit')} iconSize={16} class="bg-surface-3! hover:bg-surface-4!" onclick={() => (editing = comment.id)} />
                          <IconButton icon={Trash2} label={t('components.diff.delete')} iconSize={16} class="bg-danger-wash! text-danger! hover:bg-danger/25!" onclick={() => ask(comment)} />
                        </div>
                      {/if}
                    </li>
                  {/each}
                </ul>
              </section>
            {/each}
          </div>

          <label class="flex shrink-0 flex-col gap-1.5 pt-3 text-xs text-fg-3">
            {t('changes.review.note')}
            <textarea
              value={note}
              oninput={(event) => review.setNote(project, event.currentTarget.value)}
              rows="2"
              placeholder={t('changes.review.notePlaceholder')}
              class="w-full resize-none rounded-md border border-line bg-surface-2 px-3 py-2 text-base font-normal text-fg outline-none placeholder:text-fg-3 focus:border-focus"
            ></textarea>
            {#if noteTooLong}<span class="text-danger" role="alert">{t('changes.review.noteTooLong', { kib: MAX_TEXT_BYTES / 1024 })}</span>{/if}
          </label>
        </div>
      </div>
    {/if}

    <footer class="flex items-center gap-2 border-t border-line px-4 py-3">
      <Button variant="dangerQuiet" onclick={() => (confirmDiscard = true)}>{t('changes.review.discardAll')}</Button>
      <span class="flex-1"></span>
      <Button variant="primary" disabled={noteTooLong} onclick={onpreview}>{t('changes.review.preview')}</Button>
    </footer>
  </div>
</Overlay>

<ConfirmDialog
  bind:open={confirmDelete}
  title={t('components.diff.deleteTitle')}
  description={t('components.diff.deleteBody')}
  subject={deleting ? { icon: MessageSquare, name: deleting.body, details: `${deleting.path} · ${commentHeading(deleting)}` } : undefined}
  confirmLabel={t('components.diff.deleteConfirm')}
  onconfirm={() => deleting && review.remove(project, deleting.id)}
/>

<ConfirmDialog
  bind:open={confirmDiscard}
  title={t('changes.review.discardTitle')}
  description={`${tn('changes.review.discardDescription', comments.length)} ${t('changes.review.discardBody')}`}
  confirmLabel={t('changes.review.discard')}
  cancelLabel={t('changes.review.keep')}
  onconfirm={discard}
/>
