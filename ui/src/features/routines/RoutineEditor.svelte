<script lang="ts">
  import { untrack } from 'svelte';

  import { autofocus } from '$lib/a11y/autofocus';
  import Dialog from '$lib/components/Dialog.svelte';
  import DocFormTail from '$lib/components/docs/DocFormTail.svelte';
  import DocEditorFooter from '$lib/components/docs/DocEditorFooter.svelte';
  import { scopeIsFull } from '$lib/components/docs/doc-groups';
  import { docTarget, heldIn, sameTarget } from '$lib/components/docs/doc-target';
  import DocField from '$lib/components/docs/DocField.svelte';
  import { fieldClass } from '$lib/components/docs/doc-field';
  import ScopeSelect from '$lib/components/scope/ScopeSelect.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { RoutineView } from '$lib/ipc/bindings/RoutineView';
  import { IpcFailure } from '$lib/ipc/transport';
  import { projectScopes } from '$lib/stores/project-scopes.svelte';
  import { uid } from '$lib/utils/ids';

  import ActionFields from './ActionFields.svelte';
  import { blankForm, draftOf, type EditorTarget, type FieldKey, formOf, type Problem, problemsOf, type RoutineForm, targetOf, withTarget } from './routine-form';
  import { folderName, routineDoc } from './routine-format';
  import { routines } from './routines-data.svelte';
  import WhenFields from './WhenFields.svelte';

  interface Props {
    open: boolean;
    target: EditorTarget;
    /** The routine was written; the editor is closing. */
    onsaved: (view: RoutineView, created: boolean) => void;
  }

  let { open = $bindable(false), target, onsaved }: Props = $props();

  const ids = { name: uid('routine-name'), description: uid('routine-description'), scope: uid('routine-scope') };

  let form = $state<RoutineForm>(blankForm());
  let touched = $state<Partial<Record<FieldKey, boolean>>>({});
  let saving = $state(false);
  let failure = $state<string | null>(null);
  let previewProblem = $state<string | null>(null);

  const editing = $derived(target.view);
  const routine = $derived(editing?.routine ?? null);
  const projects = $derived(projectScopes.data ?? []);

  // The editor opens on what it was given, fresh each time.
  $effect.pre(() => {
    if (!open) {
      return;
    }

    const toEdit = target.view?.routine ?? null;

    form = toEdit ? untrack(() => formOf(toEdit, projects)) : blankForm(target.target);
    touched = {};
    failure = null;
    saving = false;
    previewProblem = null;
  });

  // The project the routine is for: its name for the help. A folder that is no project of the list any more stands as it is.
  const chosen = $derived(form.projectPath === null ? undefined : projects.find((candidate) => candidate.path === form.projectPath));
  const scopeHelp = $derived(form.scope === 'global' ? t('routines.editor.scope.global.hint') : t('routines.editor.scope.project.hint', { project: chosen?.name ?? folderName(form.projectPath ?? '') }));

  // Whether the scope the routine would be in already holds the most it can. A routine that stays in its scope is held already.
  const stays = $derived(editing !== null && sameTarget(docTarget(routineDoc(editing), projects), targetOf(form)));
  const scopeFull = $derived(scopeIsFull(heldIn(routines.views, targetOf(form), projects, routineDoc), stays));

  const problems = $derived(problemsOf(form, { projectChosen: form.projectPath !== null, scopeFull }));
  const shown = $derived(problems.filter((problem) => !problem.required || touched[problem.field]));
  const blocked = $derived(problems.length > 0 || previewProblem !== null);

  function problemOf(field: FieldKey): Problem | undefined {
    return shown.find((problem) => problem.field === field);
  }

  function messageOf(field: FieldKey): string | null {
    const problem = problemOf(field);

    return problem ? t(problem.message, problem.params) : null;
  }

  function touch(field: FieldKey): void {
    touched = { ...touched, [field]: true };
  }

  async function submit(): Promise<void> {
    touched = { name: true, cron: true, prompt: true, command: true, script: true };

    if (blocked || saving) {
      return;
    }

    saving = true;
    failure = null;

    try {
      const saved = await routines.save(draftOf(form, editing));

      open = false;
      onsaved(saved, editing === null);
    } catch (error) {
      const cause = IpcFailure.from(error);

      failure = cause.code === 'conflict' ? t('routines.toast.conflict') + ' ' + t('routines.toast.conflictHint') : cause.message;
    } finally {
      saving = false;
    }
  }
</script>

<Dialog bind:open size="lg" title={t(editing ? 'routines.editor.title.edit' : 'routines.editor.title.new')} description={t(editing ? 'routines.editor.description.edit' : 'routines.editor.description.new')}>
  <form
    class="flex flex-col gap-5"
    onsubmit={(event) => {
      event.preventDefault();
      void submit();
    }}
  >
    <DocField label={t('routines.field.name')} for={ids.name} error={messageOf('name')}>
      <input
        id={ids.name}
        bind:value={form.name}
        use:autofocus
        onblur={() => touch('name')}
        placeholder={t('routines.editor.name.placeholder')}
        autocomplete="off"
        spellcheck="false"
        aria-invalid={problemOf('name') ? 'true' : undefined}
        class={fieldClass(problemOf('name') !== undefined)}
      />
    </DocField>

    <DocField label={t('routines.field.description')} for={ids.description} aside={t('routines.editor.optional')} error={messageOf('description')}>
      <input
        id={ids.description}
        bind:value={form.description}
        placeholder={t('routines.editor.description.placeholder')}
        autocomplete="off"
        aria-invalid={problemOf('description') ? 'true' : undefined}
        class={fieldClass(problemOf('description') !== undefined)}
      />
    </DocField>

    <WhenFields bind:form {problemOf} {touch} bind:previewProblem />

    <ActionFields bind:form {problemOf} {touch} />

    <ScopeSelect label={t('routines.field.appliesTo')} labelId={ids.scope} {projects} value={targetOf(form)} onchange={(next) => (form = withTarget(form, next))} help={scopeHelp} error={messageOf('scope')} />

    <DocFormTail {failure} saveLabel={t('routines.editor.save')} />
  </form>

  {#snippet footer()}
    <DocEditorFooter cancelLabel={t('routines.editor.cancel')} saveLabel={t(editing ? 'routines.editor.save' : 'routines.editor.add')} {saving} {blocked} oncancel={() => (open = false)} onsave={() => void submit()} />
  {/snippet}
</Dialog>
