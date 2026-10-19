<script lang="ts">
  import { slide } from 'svelte/transition';

  import { autofocus } from '$lib/a11y/autofocus';
  import Dialog from '$lib/components/Dialog.svelte';
  import DocFormTail from '$lib/components/docs/DocFormTail.svelte';
  import DocEditorFooter from '$lib/components/docs/DocEditorFooter.svelte';
  import { scopeIsFull } from '$lib/components/docs/doc-groups';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import { scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HookScope } from '$lib/ipc/bindings/HookScope';
  import type { HookView } from '$lib/ipc/bindings/HookView';
    import { IpcFailure } from '$lib/ipc/transport';
  import { project } from '$lib/stores/project.svelte';
  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  import EventSelect from './EventSelect.svelte';
  import { ACTION_ICONS, ACTION_LABELS, EVENT_WHEN, length, SCOPE_ICONS, SCOPE_LABELS } from './hook-format';
  import { blankForm, DEFAULT_TIMEOUT_SECS, draftOf, type EditorTarget, type FieldKey, formOf, type HookForm, LIMITS, type Problem, problemsOf, validActions, withEvent } from './hook-form';
  import { hooks } from './hooks-data.svelte';

  interface Props {
    open: boolean;
    target: EditorTarget;
    /** The hook was written; the editor is closing. */
    onsaved: (view: HookView, created: boolean) => void;
  }

  let { open = $bindable(false), target, onsaved }: Props = $props();

  const ids = { name: uid('hook-name'), description: uid('hook-description'), event: uid('hook-event'), matcher: uid('hook-matcher'), action: uid('hook-action'), text: uid('hook-text'), timeout: uid('hook-timeout'), scope: uid('hook-scope') };

  const FIELD = 'w-full rounded-md border bg-surface-2 text-base text-fg outline-none placeholder:text-fg-3 focus:border-focus';

  let form = $state<HookForm>(blankForm());
  let touched = $state<Partial<Record<FieldKey, boolean>>>({});
  let saving = $state(false);
  let failure = $state<string | null>(null);

  const editing = $derived(target.view);
  const hook = $derived(editing?.hook ?? null);
  const projectPath = $derived(project.active?.path ?? null);

  // The editor opens on what it was given, fresh each time.
  $effect.pre(() => {
    if (!open) {
      return;
    }

    const hookToEdit = target.view?.hook ?? null;

    form = hookToEdit ? formOf(hookToEdit) : { ...withEvent(blankForm(target.scope), target.event ?? 'sessionStart') };
    touched = {};
    failure = null;
    saving = false;
  });

  // Whether the scope the hook would be in already holds the most it can: a hook that stays in its scope is not counted twice.
  const scopeFull = $derived(scopeIsFull(hooks.views.filter((view) => view.hook?.scope === form.scope).length, hook?.scope === form.scope));

  const problems = $derived(problemsOf(form, { projectOpen: projectPath !== null, scopeFull }));
  const shown = $derived(problems.filter((problem) => !problem.required || touched[problem.field]));
  const actions = $derived<StripOption<string>[]>(validActions(form.event).map((kind) => ({ value: kind, label: t(ACTION_LABELS[kind]), icon: ACTION_ICONS[kind] })));
  const scopes = $derived<StripOption<HookScope>[]>(
    (['global', 'project'] as const).map((scope) => ({ value: scope, label: t(SCOPE_LABELS[scope]), icon: SCOPE_ICONS[scope], disabled: scope === 'project' && projectPath === null })),
  );
  const isCommand = $derived(form.kind === 'command');
  const textField = $derived<FieldKey>(isCommand ? 'command' : 'text');

  function problemOf(field: FieldKey): Problem | undefined {
    return shown.find((problem) => problem.field === field);
  }

  function touch(field: FieldKey): void {
    touched = { ...touched, [field]: true };
  }

  /** The characters used of a field's limit, said once the field is nearly full. */
  function counter(value: string, max: number): string | null {
    const used = length(value.trim());

    return used >= max * 0.75 ? t('hooks.editor.counter', { used, max }) : null;
  }

  async function submit(): Promise<void> {
    touched = { name: true, text: true, command: true };

    if (problems.length > 0 || saving) {
      return;
    }

    saving = true;
    failure = null;

    try {
      const saved = await hooks.save(draftOf(form, editing, projectPath));

      open = false;
      onsaved(saved, editing === null);
    } catch (error) {
      const cause = IpcFailure.from(error);

      failure = cause.code === 'conflict' ? t('hooks.toast.conflict') + ' ' + t('hooks.toast.conflictHint') : cause.message;
    } finally {
      saving = false;
    }
  }
</script>

{#snippet error(field: FieldKey)}
  {@const problem = problemOf(field)}

  {#if problem}
    <p class="text-xs text-danger" role="alert">{t(problem.message, problem.params)}</p>
  {/if}
{/snippet}

<Dialog bind:open size="lg" title={t(editing ? 'hooks.editor.title.edit' : 'hooks.editor.title.new')} description={t(editing ? 'hooks.editor.description.edit' : 'hooks.editor.description.new')}>
  <form
    class="flex flex-col gap-5"
    onsubmit={(event) => {
      event.preventDefault();
      void submit();
    }}
  >
    <div class="flex flex-col gap-1.5">
      <label for={ids.name} class="text-sm text-fg-2">{t('hooks.field.name')}</label>
      <input
        id={ids.name}
        bind:value={form.name}
        use:autofocus
        onblur={() => touch('name')}
        placeholder={t('hooks.editor.name.placeholder')}
        autocomplete="off"
        spellcheck="false"
        aria-invalid={problemOf('name') ? 'true' : undefined}
        class={cx(FIELD, 'h-control px-3', problemOf('name') ? 'border-danger' : 'border-line-strong')}
      />
      {@render error('name')}
    </div>

    <div class="flex flex-col gap-1.5">
      <div class="flex items-baseline justify-between text-sm text-fg-2">
        <label for={ids.description}>{t('hooks.field.description')}</label>
        <small class="text-xs text-fg-3">{t('hooks.editor.optional')}</small>
      </div>
      <input
        id={ids.description}
        bind:value={form.description}
        placeholder={t('hooks.editor.description.placeholder')}
        autocomplete="off"
        aria-invalid={problemOf('description') ? 'true' : undefined}
        class={cx(FIELD, 'h-control px-3', problemOf('description') ? 'border-danger' : 'border-line-strong')}
      />
      {@render error('description')}
    </div>

    <div class="flex flex-col gap-1.5">
      <span id={ids.event} class="text-sm text-fg-2">{t('hooks.editor.event')}</span>
      <EventSelect value={form.event} labelledby={ids.event} onchange={(event) => (form = withEvent(form, event))} />
    </div>

    {#if form.event === 'toolEnd'}
      <div transition:slide={{ duration: scaled(200) }} class="-m-1 overflow-hidden p-1">
        <div class="flex flex-col gap-1.5">
          <div class="flex items-baseline justify-between text-sm text-fg-2">
            <label for={ids.matcher}>{t('hooks.field.toolFilter')}</label>
            <small class="text-xs text-fg-3">{t('hooks.editor.optional')}</small>
          </div>
          <input
            id={ids.matcher}
            bind:value={form.matcher}
            placeholder="edit|write"
            autocomplete="off"
            spellcheck="false"
            aria-invalid={problemOf('matcher') ? 'true' : undefined}
            aria-describedby="{ids.matcher}-hint"
            class={cx(FIELD, 'h-control px-3 font-mono', problemOf('matcher') ? 'border-danger' : 'border-line-strong')}
          />
          {#if problemOf('matcher')}
            <span id="{ids.matcher}-hint">{@render error('matcher')}</span>
          {:else}
            <p id="{ids.matcher}-hint" class="text-xs text-fg-3">{t('hooks.editor.matcher.hint')}</p>
          {/if}
        </div>
      </div>
    {/if}

    <div class="flex flex-col gap-1.5">
      <span id={ids.action} class="text-sm text-fg-2">{t('hooks.editor.action')}</span>
      <div class="flex"><SegmentedControl label={t('hooks.editor.action')} options={actions} bind:value={() => form.kind, (next) => (form.kind = next as HookForm['kind'])} /></div>
      <p class="text-xs text-fg-3">{t(isCommand ? 'hooks.editor.action.command.hint' : form.kind === 'followUp' ? 'hooks.editor.action.followUp.hint' : 'hooks.editor.action.addContext.hint')}</p>
    </div>

    <div class="flex flex-col gap-1.5">
      <div class="flex items-baseline justify-between gap-3 text-sm text-fg-2">
        <label for={ids.text}>{t(isCommand ? 'hooks.field.command' : form.kind === 'followUp' ? 'hooks.field.message' : 'hooks.field.context')}</label>
        <small class="text-xs text-fg-3">{counter(isCommand ? form.command : form.text, isCommand ? LIMITS.command : LIMITS.text) ?? t(isCommand ? 'hooks.editor.command.sub' : form.kind === 'followUp' ? 'hooks.editor.message.sub' : 'hooks.editor.context.sub')}</small>
      </div>
      {#if isCommand}
        <textarea
          id={ids.text}
          bind:value={form.command}
          onblur={() => touch('command')}
          rows="3"
          placeholder="bun run lint --quiet"
          spellcheck="false"
          aria-invalid={problemOf('command') ? 'true' : undefined}
          class={cx(FIELD, 'resize-none px-3 py-2 font-mono text-sm', problemOf('command') ? 'border-danger' : 'border-line-strong')}
        ></textarea>
        <p class="text-xs text-fg-3">{t('hooks.editor.command.quote')}</p>
      {:else}
        <textarea
          id={ids.text}
          bind:value={form.text}
          onblur={() => touch('text')}
          rows="4"
          placeholder={t(form.kind === 'followUp' ? 'hooks.editor.message.placeholder' : 'hooks.editor.context.placeholder')}
          aria-invalid={problemOf('text') ? 'true' : undefined}
          class={cx(FIELD, 'resize-none px-3 py-2', problemOf('text') ? 'border-danger' : 'border-line-strong')}
        ></textarea>
      {/if}
      {@render error(textField)}
    </div>

    {#if isCommand}
      <div transition:slide={{ duration: scaled(200) }} class="-m-1 overflow-hidden p-1">
        <div class="flex flex-col gap-1.5">
          <label for={ids.timeout} class="text-sm text-fg-2">{t('hooks.field.timeout')}</label>
          <div class="flex items-center gap-2">
            <input
              id={ids.timeout}
              bind:value={form.timeout}
              inputmode="numeric"
              autocomplete="off"
              placeholder={String(DEFAULT_TIMEOUT_SECS)}
              aria-invalid={problemOf('timeout') ? 'true' : undefined}
              class={cx(FIELD, 'h-control w-28 px-3', problemOf('timeout') ? 'border-danger' : 'border-line-strong')}
            />
            <span class="text-sm text-fg-3">{t('hooks.editor.timeout.unit')}</span>
          </div>
          {#if problemOf('timeout')}{@render error('timeout')}{:else}<p class="text-xs text-fg-3">{t('hooks.editor.timeout.hint')}</p>{/if}
        </div>
      </div>
    {/if}

    <div class="flex flex-col gap-1.5">
      <span id={ids.scope} class="text-sm text-fg-2">{t('hooks.editor.scope')}</span>
      <div class="flex"><SegmentedControl label={t('hooks.editor.scope')} options={scopes} bind:value={() => form.scope, (next) => (form.scope = next)} /></div>
      <p class="text-xs text-fg-3">{t(form.scope === 'global' ? 'hooks.editor.scope.global.hint' : 'hooks.editor.scope.project.hint', { project: project.active?.name ?? '' })}</p>
      {@render error('scope')}
    </div>

    <DocFormTail {failure} saveLabel={t('hooks.editor.save')} />
  </form>

  {#snippet footer()}
    <DocEditorFooter cancelLabel={t('hooks.editor.cancel')} saveLabel={t(editing ? 'hooks.editor.save' : 'hooks.editor.add')} {saving} blocked={problems.length > 0} oncancel={() => (open = false)} onsave={() => void submit()} />
  {/snippet}
</Dialog>
