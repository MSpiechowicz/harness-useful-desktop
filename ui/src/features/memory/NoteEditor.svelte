<script lang="ts">
  import Lock from '@lucide/svelte/icons/lock';
  import { untrack } from 'svelte';

  import DocChips from '$lib/components/docs/DocChips.svelte';
  import DocField from '$lib/components/docs/DocField.svelte';
  import { fieldClass } from '$lib/components/docs/doc-field';
  import DocSelect from '$lib/components/docs/DocSelect.svelte';
  import { ICON } from '$lib/components/icon';
  import { targetValue } from '$lib/components/scope/scope-options';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { NoteContent } from '$lib/ipc/bindings/NoteContent';
  import type { NoteLink } from '$lib/ipc/bindings/NoteLink';
  import type { NoteSaved } from '$lib/ipc/bindings/NoteSaved';
  import { saveNote } from '$lib/stores/memoryNotes.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { projectScopes } from '$lib/stores/project-scopes.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { uid } from '$lib/utils/ids';

  import { fieldProblems, onOpen } from './editor-fields.svelte';
  import EditorDialog from './EditorDialog.svelte';
  import EditorScope from './EditorScope.svelte';
  import ExistsNotice from './ExistsNotice.svelte';
  import { KIND_LABELS, NOTE_KINDS } from './note-format';
  import NoteLinks from './NoteLinks.svelte';
  import { defaultTarget, editProject, projectOfTarget, targetLabel, targetOfItem } from './scope-target';
  import { spelledOut } from './script-format';
  import {
    blankForm,
    draftOf,
    ENTITY_RULES,
    entitiesOf,
    entityFlagged,
    type FieldKey,
    formOf,
    length,
    LIMITS,
    type NoteForm,
    problemsOf,
    TAG_RULES,
    tagFlagged,
  } from './note-form';

  interface Props {
    open: boolean;
    /** The note to edit; null adds a new one. */
    note: NoteContent | null;
    /** The links of the note edited, as its panel read them; shown, never changed. */
    links: NoteLink[];
    /** The note was written; the editor is closing. */
    onsaved: (saved: NoteSaved, title: string) => void;
    /** The user chose to open the note that has the title instead of saving. The editor is closing. */
    onopen: (id: number) => void;
  }

  let { open = $bindable(false), note, links, onsaved, onopen }: Props = $props();

  const FIELDS: readonly FieldKey[] = ['title', 'body', 'tags', 'entities', 'scope'];

  const ids = { title: uid('note-title'), body: uid('note-body'), kind: uid('note-kind'), tags: uid('note-tags'), entities: uid('note-entities'), links: uid('note-links'), kept: uid('note-kept'), scope: uid('note-scope') };

  /** What a save found: this note (of this title, in this scope) already has the title. `target` is the Applies to choice the save was for, as `targetValue` gives it. */
  interface Taken {
    id: number;
    title: string;
    target: string;
  }

  let form = $state<NoteForm>(blankForm({ kind: 'global' }));
  let saving = $state(false);
  let taken = $state<Taken | null>(null);
  let failure = $state<string | null>(null);

  const editing = $derived(note !== null);
  const projectPath = $derived(project.active?.path ?? null);
  const projects = $derived(projectScopes.data ?? []);

  const fields = fieldProblems(() => problemsOf(form, { editing }), () => open);

  // The editor opens on what it was given, fresh each time.
  onOpen(
    () => open,
    () => {
      form = note ? untrack(() => formOf(note, projects)) : blankForm(untrack(() => defaultTarget(viewState.memoryReach, projects, projectPath)));
      taken = null;
      failure = null;
    },
  );

  // An item that exists keeps its scope, but the project it is shown as may be known only after the editor opened (the projects load on their own).
  $effect(() => {
    if (note !== null && open) {
      form.target = targetOfItem(note, projects);
    }
  });

  // The finding stands only while the draft is still the one it was found for.
  const conflict = $derived(taken && taken.title === form.title.trim() && taken.target === targetValue(form.target) ? taken : null);
  // Replacing goes through an edit, which leaves the entities of the note it replaces as they are.
  const existsBody = $derived.by(() => {
    if (editing) {
      return t('memory.editor.note.exists.body.edit', { title: conflict?.title ?? '' });
    }

    const body = t('memory.editor.note.exists.body', { title: conflict?.title ?? '' });

    return entitiesOf(form).length > 0 ? `${body} ${t('memory.editor.note.exists.entities')}` : body;
  });
  const kinds = $derived(NOTE_KINDS.map((kind) => ({ value: kind, label: t(KIND_LABELS[kind]) })));
  const scopeHelp = $derived.by(() => {
    if (editing) {
      return t('memory.editor.note.scope.fixed');
    }

    return form.target.kind === 'repo' ? t('memory.editor.note.scope.repo.hint', { project: targetLabel(projects, form.target, t('components.scope.global')) }) : t('memory.editor.note.scope.global.hint');
  });

  /** The characters used of a field's limit, said once the field is nearly full. */
  function counter(value: string, max: number): string | undefined {
    const used = length(value.trim());

    return used >= max * 0.75 ? t('memory.editor.counter', { used, max }) : undefined;
  }

  /** Writes the note. `replaceId` is the note that has the title, when the user chose to replace it. */
  async function submit(replaceId?: number): Promise<void> {
    fields.touchAll(FIELDS);

    if (fields.blocked || saving) {
      return;
    }

    saving = true;
    failure = null;

    try {
      const draft = draftOf(form);
      const target = targetValue(form.target);
      const saved = await saveNote(note ? editProject(note, projects, projectPath) : projectOfTarget(form.target), draft, replaceId ?? note?.id);

      if (saved === null) {
        return;
      }

      if (saved.outcome === 'exists') {
        // A server that does not say which note has the title leaves nothing to open or replace: the user can only rename.
        if (saved.id === null) {
          failure = t(editing ? 'memory.editor.note.exists.body.edit' : 'memory.editor.note.exists.body', { title: draft.title });
        } else {
          taken = { id: saved.id, title: draft.title, target };
        }

        return;
      }

      open = false;
      onsaved(saved, draft.title);
    } finally {
      saving = false;
    }
  }

  function openTaken(id: number): void {
    open = false;
    onopen(id);
  }
</script>


<!-- The focus starts on the title. -->
<EditorDialog
  bind:open
  title={t(editing ? 'memory.editor.note.title.edit' : 'memory.editor.note.title.new')}
  description={t(editing ? 'memory.editor.note.description.edit' : 'memory.editor.note.description.new')}
  initialFocus={`#${ids.title}`}
  saveLabel={t(editing ? 'memory.editor.note.save' : 'memory.editor.note.add')}
  {saving}
  blocked={fields.blocked}
  {failure}
  onsubmit={() => void submit()}
>
  <DocField label={t('memory.editor.note.field.title')} for={ids.title} aside={counter(form.title, LIMITS.title)} error={fields.messageOf('title')}>
    <input
      id={ids.title}
      bind:value={form.title}
      onblur={() => fields.touch('title')}
      placeholder={t('memory.editor.note.title.placeholder')}
      autocomplete="off"
      aria-invalid={fields.messageOf('title') ? 'true' : undefined}
      class={fieldClass(fields.messageOf('title') !== null)}
    />
  </DocField>

  <!-- Replacing is for adding: an edited note renamed onto another would overwrite it and leave the edited one behind. -->
  {#if conflict}
    <ExistsNotice
      title={t('memory.editor.note.exists.title')}
      body={existsBody}
      replacing={saving}
      onopen={() => openTaken(conflict.id)}
      onreplace={editing ? undefined : () => void submit(conflict.id)}
    />
  {/if}

  <DocField label={t('memory.editor.note.field.body')} for={ids.body} aside={counter(form.body, LIMITS.body)} error={fields.messageOf('body')}>
    <textarea
      id={ids.body}
      bind:value={form.body}
      onblur={() => fields.touch('body')}
      rows="6"
      placeholder={t('memory.editor.note.body.placeholder')}
      aria-invalid={fields.messageOf('body') ? 'true' : undefined}
      class={fieldClass(fields.messageOf('body') !== null, 'resize-none px-3 py-2')}
    ></textarea>
  </DocField>

  <DocField label={t('memory.editor.note.field.kind')} labelId={ids.kind}>
    <DocSelect label={t('memory.editor.note.field.kind')} options={kinds} value={form.kind} labelledby={ids.kind} onchange={(next) => (form.kind = next)} />
  </DocField>

  <DocField label={t('memory.editor.note.field.tags')} for={ids.tags} aside={t('memory.editor.optional')} error={fields.messageOf('tags')} help={t('memory.editor.note.tags.help')}>
    <DocChips
      id={ids.tags}
      bind:values={form.tags}
      bind:draft={form.tagsDraft}
      rules={TAG_RULES}
      removeLabel="memory.editor.note.tags.remove"
      placeholder={t('memory.editor.note.tags.placeholder')}
      flagged={tagFlagged}
      invalid={fields.messageOf('tags') !== null}
      onblur={() => fields.touch('tags')}
    />
  </DocField>

  <!-- The server's note_update takes neither entities nor links, so an edit shows them and takes none. -->
  {#if editing}
    <section class="flex flex-col gap-4 rounded-md border border-line p-4" aria-labelledby={ids.kept}>
      <p id={ids.kept} class="flex items-start gap-2 text-sm text-fg-2">
        <Lock size={ICON.sm} class="mt-0.5 shrink-0 text-fg-3" aria-hidden="true" />
        {t('memory.editor.note.kept')}
      </p>

      <div class="flex flex-col gap-2">
        <p id={ids.entities} class="text-sm font-medium text-fg">{t('memory.editor.note.field.entities')}</p>
        {#if form.entities.length === 0}
          <p class="text-sm text-fg-3">{t('memory.editor.note.entities.none')}</p>
        {:else}
          <ul class="flex flex-wrap gap-2" aria-labelledby={ids.entities}>
            {#each form.entities as entity (entity)}
              <li class="rounded-sm bg-surface-3 px-2.5 py-1 font-mono text-sm text-fg">{spelledOut(entity)}</li>
            {/each}
          </ul>
        {/if}
      </div>

      <div class="flex flex-col gap-1">
        <p id={ids.links} class="text-sm font-medium text-fg">{t('memory.notes.detail.links')}</p>
        <NoteLinks {links} labelledby={ids.links} />
      </div>
    </section>
  {:else}
    <DocField label={t('memory.editor.note.field.entities')} for={ids.entities} aside={t('memory.editor.optional')} error={fields.messageOf('entities')} help={t('memory.editor.note.entities.help')}>
      <DocChips
        id={ids.entities}
        bind:values={form.entities}
        bind:draft={form.entitiesDraft}
        rules={ENTITY_RULES}
        removeLabel="memory.editor.note.entities.remove"
        placeholder={t('memory.editor.note.entities.placeholder')}
        mono
        flagged={entityFlagged}
        invalid={fields.messageOf('entities') !== null}
        onblur={() => fields.touch('entities')}
      />
    </DocField>
  {/if}

  <EditorScope bind:value={form.target} {projects} id={ids.scope} error={fields.messageOf('scope')} help={scopeHelp} fixed={editing} />
</EditorDialog>
