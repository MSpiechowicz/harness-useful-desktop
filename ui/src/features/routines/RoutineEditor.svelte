<script lang="ts">
  import { autofocus } from '$lib/a11y/autofocus';
  import Dialog from '$lib/components/Dialog.svelte';
  import DocFormTail from '$lib/components/docs/DocFormTail.svelte';
  import DocEditorFooter from '$lib/components/docs/DocEditorFooter.svelte';
  import { scopeIsFull } from '$lib/components/docs/doc-groups';
  import DocField from '$lib/components/docs/DocField.svelte';
  import { fieldClass } from '$lib/components/docs/doc-field';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HookScope } from '$lib/ipc/bindings/HookScope';
  import type { RoutineView } from '$lib/ipc/bindings/RoutineView';
  import { IpcFailure } from '$lib/ipc/transport';
  import { project } from '$lib/stores/project.svelte';
  import { uid } from '$lib/utils/ids';

  import ActionFields from './ActionFields.svelte';
  import { blankForm, draftOf, type EditorTarget, type FieldKey, formOf, type Problem, problemsOf, type RoutineForm, withScope } from './routine-form';
  import { SCOPE_ICONS, SCOPE_LABELS } from './routine-format';
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
  const projectPath = $derived(project.active?.path ?? null);

  // The editor opens on what it was given, fresh each time.
  $effect.pre(() => {
    if (!open) {
      return;
    }

    const toEdit = target.view?.routine ?? null;

    form = toEdit ? formOf(toEdit) : blankForm(target.scope);
    touched = {};
    failure = null;
    saving = false;
    previewProblem = null;
  });

  // Whether the scope the routine would be in already holds the most it can.
  const scopeFull = $derived(scopeIsFull(routines.views.filter((view) => view.origin === 'local' && view.routine?.scope === form.scope).length, routine?.scope === form.scope));

  const problems = $derived(problemsOf(form, { projectOpen: projectPath !== null, scopeFull }));
  const shown = $derived(problems.filter((problem) => !problem.required || touched[problem.field]));
  const blocked = $derived(problems.length > 0 || previewProblem !== null);
  const scopes = $derived<StripOption<HookScope>[]>(
    (['global', 'project'] as const).map((scope) => ({ value: scope, label: t(SCOPE_LABELS[scope]), icon: SCOPE_ICONS[scope], disabled: scope === 'project' && projectPath === null })),
  );

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
      const saved = await routines.save(draftOf(form, editing, projectPath));

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

    <ActionFields bind:form {problemOf} {touch} {projectPath} />

    <DocField label={t('routines.editor.scope')} labelId={ids.scope} error={messageOf('scope')} help={t(form.scope === 'global' ? 'routines.editor.scope.global.hint' : 'routines.editor.scope.project.hint', { project: project.active?.name ?? '' })}>
      <div class="flex"><SegmentedControl label={t('routines.editor.scope')} options={scopes} bind:value={() => form.scope, (next) => (form = withScope(form, next))} /></div>
    </DocField>

    <DocFormTail {failure} saveLabel={t('routines.editor.save')} />
  </form>

  {#snippet footer()}
    <DocEditorFooter cancelLabel={t('routines.editor.cancel')} saveLabel={t(editing ? 'routines.editor.save' : 'routines.editor.add')} {saving} {blocked} oncancel={() => (open = false)} onsave={() => void submit()} />
  {/snippet}
</Dialog>
