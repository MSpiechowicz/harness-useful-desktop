<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import Pencil from '@lucide/svelte/icons/pencil';
  import X from '@lucide/svelte/icons/x';

  import Button from '$lib/components/Button.svelte';
  import { editedNote, editedScript, heldView } from '$lib/chat/memory-held';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HeldItem } from '$lib/ipc/bindings/HeldItem';
  import type { MemoryNotesRouting } from '$lib/ipc/bindings/MemoryNotesRouting';
  import type { NoteDraft } from '$lib/ipc/bindings/NoteDraft';
  import type { ScriptDraft } from '$lib/ipc/bindings/ScriptDraft';
  import { memoryHeld } from '$lib/stores/memoryHeld.svelte';
  import { noteList } from '$lib/stores/memoryNotes.svelte';
  import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
  import { scriptList } from '$lib/stores/memoryScripts.svelte';
  import { uid } from '$lib/utils/ids';

  import EditorSavePlace from '../memory/EditorSavePlace.svelte';
  import NoteEditor from '../memory/NoteEditor.svelte';
  import { SavePlace } from '../memory/save-place.svelte';
  import ScriptEditor from '../memory/ScriptEditor.svelte';
  import { heldPlaces } from './held-places';
  import MemoryHeldNote from './MemoryHeldNote.svelte';
  import MemoryHeldScript from './MemoryHeldScript.svelte';
  import { canEditHeld, heldProjectOf, noteContentOf, noteFormOf, scriptFormOf } from './memory-held-edit';

  interface Props {
    item: HeldItem;
  }

  let { item }: Props = $props();

  let editing = $state(false);
  /** The version of the save the editor was opened on: what an edited save is sent for, even if the agent holds it again meanwhile. */
  let editedRevision: string | null = null;

  const view = $derived(heldView(item));
  // The edited save is written in the chat's project whatever the editor shows, so the editor offers that one only.
  const pinned = $derived({ path: heldProjectOf(item) });
  const busy = $derived(memoryHeld.busy.has(item.heldId));
  const editable = $derived(canEditHeld(view));
  const failure = $derived(memoryHeld.failures[item.heldId] ?? null);

  // Where a new note or script goes, for the scope it is kept in. The card and its Edit dialog keep this one value. A change to a saved note
  // (`note_update`) and a script that replaces one keep the place of what they change, so the field never shows for them.
  const placeId = uid('held-place');
  /** The scope the open editor shows, while it is open: the save goes where that scope is kept. */
  let editedScope = $state<'repo' | 'global' | null>(null);

  const place = new SavePlace(() => {
    const remote = memoryRemote.value;
    const current = view;

    return {
      adding: current.type === 'note' ? item.tool === 'note_save' && current.noteId === null : current.current === null,
      remote,
      supported: remote?.heldSavePlaceSupported ?? false,
      routing: routingOf(current),
      offline: (current.type === 'note' ? noteList.data?.offline : scriptList.data?.offline) ?? false,
    };
  });

  function routingOf(current: typeof view): MemoryNotesRouting {
    const remote = memoryRemote.value;

    if (current.type === 'script') {
      return remote?.scripts ?? 'remote';
    }

    const scope = (editing ? editedScope : null) ?? current.note.scope;

    return (scope === 'repo' ? remote?.repositoryNotes : remote?.globalNotes) ?? 'remote';
  }

  $effect(() => heldPlaces.register(item.heldId, place));

  $effect(() => {
    if (!editing) {
      editedScope = null;
    }
  });

  function openEditor(): void {
    if (!editable) {
      return;
    }

    editedRevision = item.revision;
    editing = true;
  }

  // The agent held the save again while the editor was open: the draft is of a version that is gone, so the editor closes and says so.
  $effect(() => {
    if (editing && item.revision !== editedRevision) {
      editing = false;
      memoryHeld.reportChanged(item.heldId);
    }
  });

  /** Saves the draft the user made in the editor; resolves with why it could not, for the editor to say. */
  async function saveEdited(draft: NoteDraft | ScriptDraft): Promise<string | null> {
    const edited = 'title' in draft ? editedNote(item, draft) : editedScript(draft);

    if (await memoryHeld.save(item, edited, editedRevision, place.sent)) {
      return null;
    }

    return memoryHeld.failures[item.heldId] ?? t('memory.guards.waiting.saveFailed');
  }
</script>

<article class="flex flex-col gap-3 border-t border-line p-4 first:border-t-0" aria-busy={busy || undefined}>
  {#if view.type === 'note'}
    <MemoryHeldNote {view} />
  {:else}
    <MemoryHeldScript {view} />
  {/if}

  {#if place.offered}
    <EditorSavePlace bind:value={place.value} id={placeId} offline={place.offline} />
  {/if}

  {#if failure}
    <p class="rounded-md border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger" role="alert">{failure}</p>
  {/if}

  <div class="flex justify-end gap-2">
    <Button variant="ghost" disabled={busy} onclick={() => void memoryHeld.skip(item)}>
      {#snippet leading()}<X size={14} aria-hidden="true" />{/snippet}
      {t('memory.guards.waiting.skip')}
    </Button>
    <Button disabled={busy || !editable} title={editable ? undefined : t('memory.guards.waiting.editUnread')} onclick={openEditor}>
      {#snippet leading()}<Pencil size={14} aria-hidden="true" />{/snippet}
      {t('memory.guards.waiting.edit')}
    </Button>
    <Button variant="primary" loading={busy} onclick={() => void memoryHeld.save(item, undefined, undefined, place.sent)}>
      {#snippet leading()}<Check size={14} aria-hidden="true" />{/snippet}
      {t('memory.guards.waiting.save')}
    </Button>
  </div>
</article>

<!-- The editor changes the draft only: what the user keeps goes to Harness Memory as the edited save, and the agent's own is dropped. -->
{#if view.type === 'note'}
  {#if view.noteId === null}
    <NoteEditor bind:open={editing} note={null} links={[]} prefill={noteFormOf(item, view)} commit={saveEdited} {pinned} {place} onscope={(scope) => (editedScope = scope)} />
  {:else}
    <NoteEditor bind:open={editing} note={noteContentOf(item, view)} links={[]} commit={saveEdited} {pinned} />
  {/if}
{:else}
  <ScriptEditor bind:open={editing} script={null} prefill={scriptFormOf(item, view)} commit={saveEdited} {pinned} {place} />
{/if}
