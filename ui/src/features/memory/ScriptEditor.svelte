<script lang="ts">
  import ShieldCheck from '@lucide/svelte/icons/shield-check';
  import { untrack } from 'svelte';

  import DocField from '$lib/components/docs/DocField.svelte';
  import { fieldClass } from '$lib/components/docs/doc-field';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import { targetValue } from '$lib/components/scope/scope-options';
  import type { StripOption } from '$lib/components/strip';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { ScriptDetail } from '$lib/ipc/bindings/ScriptDetail';
  import type { ScriptDraft } from '$lib/ipc/bindings/ScriptDraft';
  import type { ScriptLanguage } from '$lib/ipc/bindings/ScriptLanguage';
  import type { ScriptSaved } from '$lib/ipc/bindings/ScriptSaved';
  import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
  import { rereadScript, SCRIPT_CHANGED, saveScript, scriptList } from '$lib/stores/memoryScripts.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { projectScopes } from '$lib/stores/project-scopes.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { uid } from '$lib/utils/ids';

  import { fieldProblems, onOpen } from './editor-fields.svelte';
  import EditorDialog from './EditorDialog.svelte';
  import EditorScope from './EditorScope.svelte';
  import EditorChangedNotice from './EditorChangedNotice.svelte';
  import EditorNotice from './EditorNotice.svelte';
  import ExistsNotice from './ExistsNotice.svelte';
  import HiddenText from './HiddenText.svelte';
  import { defaultTarget, editProject, offeredProjects, type PinnedProject, pinnedHelp, projectOfTarget, targetLabel, targetOfItem } from './scope-target';
  import { OpenedVersion } from './opened-version.svelte';
  import { SavePlace } from './save-place.svelte';
  import { blankForm, codeChanged, DEFAULT_TIMEOUT_SECS, draftOf, type FieldKey, formOf, LANGUAGES, problemsOf, type ScriptForm, unsupportedLanguage } from './script-form';
  import { languageLabel, needsReview, revealHidden } from './script-format';

  interface Props {
    open: boolean;
    /** The script to edit, with its code; null adds a new one. */
    script: ScriptDetail | null;
    /** The script was written; the editor is closing. `codeChanged` tells whether the code differs from what the editor was opened on. */
    onsaved?: (saved: ScriptSaved, name: string, codeChanged: boolean) => void;
    /** The user chose to open the script that has the name instead of saving. The editor is closing. */
    onopen?: (id: number) => void;
    /** What a new script starts with instead of a blank form (the script an agent wants to save, to change before it is). */
    prefill?: ScriptForm | null;
    /**
     * Takes the draft instead of writing the script: the caller saves it. Resolves with why it could not, or null when it did, and the
     * editor closes. The editor then neither writes the script nor looks for one of the same name.
     */
    commit?: (draft: ScriptDraft) => Promise<string | null>;
    /** Set for a script an agent holds: the project it is written in, which the Applies to field offers besides Global. */
    pinned?: PinnedProject | null;
    /** The Save to field of the card the editor was opened from, so the card and the editor keep one value. Without it the editor keeps its own, and starts it again each time it opens. */
    place?: SavePlace;
  }

  let { open = $bindable(false), script, onsaved, onopen, prefill = null, commit, pinned = null, place: shared }: Props = $props();

  const FIELDS: readonly FieldKey[] = ['name', 'description', 'body', 'timeout', 'scope'];

  const ids = { name: uid('script-name'), description: uid('script-description'), language: uid('script-language'), body: uid('script-body'), timeout: uid('script-timeout'), scope: uid('script-scope') };

  /** What a save found: this script (of this name, in this scope) already exists. `target` is the Applies to choice the save was for, as `targetValue` gives it. */
  interface Taken {
    id: number;
    name: string;
    target: string;
  }

  let form = $state<ScriptForm>(blankForm({ kind: 'global' }));
  let saving = $state(false);
  let taken = $state<Taken | null>(null);
  /** The version of the script the form was filled from. Its hash is what an edit is saved against. */
  const version = new OpenedVersion<ScriptDetail>();
  let failure = $state<string | null>(null);

  const editing = $derived(script !== null);
  // A script that waits for review stays waiting unless its code is changed here, which makes it the user's own.
  const waiting = $derived(version.opened !== null && needsReview(version.opened));
  // A language this version does not know is not edited: saving would have to name another one.
  const unsupported = $derived(version.opened ? unsupportedLanguage(version.opened) : null);
  const projectPath = $derived(project.active?.path ?? null);
  const projects = $derived(projectScopes.data ?? []);
  const choices = $derived(offeredProjects(pinned, projects));

  // Where a new script goes, for the scope it is kept in: the review card that hands its draft to the caller has no say in it.
  const ownPlace = new SavePlace(() => {
    const remote = memoryRemote.value;

    return {
      adding: !editing && !commit,
      remote,
      supported: remote?.savePlaceSupported ?? false,
      routing: remote?.scripts ?? 'remote',
      offline: scriptList.data?.offline ?? false,
    };
  });
  const place = $derived(shared ?? ownPlace);

  /** What the editor opens on: the script edited, else what an agent wants to save, else a blank script in the scope the view looks at. */
  function startingForm(): ScriptForm {
    if (script) {
      return formOf(script, projects);
    }

    return prefill ? $state.snapshot(prefill) : blankForm(defaultTarget(viewState.memoryReach, projects, projectPath));
  }

  // The editor opens on what it was given, fresh each time.
  onOpen(
    () => open,
    () => {
      version.open(script);
      if (!shared) {
        place.open();
      }

      form = untrack(startingForm);
      taken = null;
      failure = null;
    },
  );

  const fields = fieldProblems(() => problemsOf(form, { editing }), () => open);

  // An item that exists keeps its scope, but the project it is shown as may be known only after the editor opened (the projects load on their own).
  $effect(() => {
    if (script !== null && open) {
      form.target = targetOfItem(script, projects);
    }
  });

  // The finding stands only while the draft is still the one it was found for.
  const conflict = $derived(taken && taken.name === form.name.trim() && taken.target === targetValue(form.target) ? taken : null);
  const hidden = $derived(revealHidden(form.body).count);
  const languages = $derived<StripOption<ScriptLanguage>[]>(LANGUAGES.map((language) => ({ value: language, label: languageLabel(language) })));
  const scopeHelp = $derived.by(() => {
    if (editing) {
      return t('memory.editor.script.scope.fixed');
    }

    const held = pinnedHelp(pinned, form.target, choices);

    if (held !== null) {
      return held;
    }

    return form.target.kind === 'repo' ? t('memory.editor.script.scope.repo.hint', { project: targetLabel(projects, form.target, t('components.scope.global')) }) : t('memory.editor.script.scope.global.hint');
  });

  /** Reads the script again and fills the form with it: the edits made on the old version are dropped. */
  function reload(): Promise<void> {
    return version.reload(
      (id) => rereadScript(id, viewState.memoryReach),
      (latest) => (form = formOf(latest, projects)),
    );
  }

  /** Writes the script. `replace` overwrites the script that has the name: an edit, or the user's choice on a taken name. */
  async function submit(replace = editing): Promise<void> {
    fields.touchAll(FIELDS);

    if (fields.blocked || saving || unsupported !== null || version.stale === 'gone') {
      return;
    }

    saving = true;

    try {
      const draft = draftOf(form);

      if (commit) {
        failure = await commit(draft);

        if (failure === null) {
          open = false;
        }

        return;
      }

      const target = targetValue(form.target);
      // An edit is saved against the version it opened, never an add, not even one that replaces the script of that name.
      // The place is for a new script only: replacing the script that has the name keeps the place that script is in.
      const saved = await saveScript(
        script ? editProject(script, projects, projectPath) : projectOfTarget(form.target),
        draft,
        replace,
        editing ? version.opened?.sha256 : undefined,
        replace ? undefined : place.sent,
      );

      if (saved === null) {
        return;
      }

      if (saved === SCRIPT_CHANGED) {
        version.stale = 'changed';

        return;
      }

      if (saved.outcome === 'exists') {
        taken = { id: saved.id, name: draft.name, target };

        return;
      }

      open = false;
      onsaved?.(saved, draft.name, version.opened === null || codeChanged(version.opened.body, draft.body));
    } finally {
      saving = false;
    }
  }

  function openTaken(id: number): void {
    open = false;
    onopen?.(id);
  }
</script>

<!-- The focus starts on the first field that can be typed in: the name of a new script, the description of an edited one. -->
<EditorDialog
  bind:open
  title={commit ? t('memory.guards.edit.script.title') : t(editing ? 'memory.editor.script.title.edit' : 'memory.editor.script.title.new')}
  description={commit ? t('memory.guards.edit.description') : t(editing ? 'memory.editor.script.description.edit' : 'memory.editor.script.description.new')}
  initialFocus={`#${editing ? ids.description : ids.name}`}
  saveLabel={commit ? t('memory.guards.waiting.save') : t(editing ? 'memory.editor.script.save' : 'memory.editor.script.add')}
  {saving}
  blocked={fields.blocked || unsupported !== null || version.stale === 'gone'}
  {failure}
  onsubmit={() => void submit()}
>
  {#if version.stale !== null}
    <EditorChangedNotice gone={version.stale === 'gone'} changedText={t('memory.editor.script.changed')} goneText={t('memory.editor.script.gone')} reloading={version.reloading} onreload={() => void reload()} onclose={() => (open = false)} />
  {/if}

  {#if waiting}
    <EditorNotice title={t('memory.scripts.trust.noticeTitle')} body={t('memory.editor.script.review.body')} />
  {/if}

  <DocField label={t('memory.editor.script.field.name')} for={ids.name} error={fields.messageOf('name')} help={t(editing ? 'memory.editor.script.name.fixed' : 'memory.editor.script.name.help')}>
    <input
      id={ids.name}
      bind:value={form.name}
      onblur={() => fields.touch('name')}
      readonly={editing}
      placeholder={t('memory.editor.script.name.placeholder')}
      autocomplete="off"
      spellcheck="false"
      aria-invalid={fields.messageOf('name') ? 'true' : undefined}
      class={fieldClass(fields.messageOf('name') !== null, 'h-control px-3', editing ? 'font-mono text-fg-2' : 'font-mono')}
    />
  </DocField>

  {#if conflict}
    <ExistsNotice
      title={t('memory.editor.script.exists.title')}
      body={t('memory.editor.script.exists.body', { name: conflict.name })}
      replacing={saving}
      onopen={() => openTaken(conflict.id)}
      onreplace={() => void submit(true)}
    />
  {/if}

  <DocField label={t('memory.editor.script.field.description')} for={ids.description} error={fields.messageOf('description')}>
    <input
      id={ids.description}
      bind:value={form.description}
      onblur={() => fields.touch('description')}
      placeholder={t('memory.editor.script.description.placeholder')}
      autocomplete="off"
      aria-invalid={fields.messageOf('description') ? 'true' : undefined}
      class={fieldClass(fields.messageOf('description') !== null)}
    />
  </DocField>

  <DocField label={t('memory.editor.script.field.language')} labelId={ids.language}>
    {#if unsupported !== null}
      <EditorNotice title={t('memory.editor.script.language.unsupported.title')} body={t('memory.editor.script.language.unsupported.body', { language: unsupported })} />
    {:else}
      <div class="flex"><SegmentedControl label={t('memory.editor.script.field.language')} options={languages} bind:value={form.language} /></div>
    {/if}
  </DocField>

  <DocField label={t('memory.editor.script.field.code')} for={ids.body} error={fields.messageOf('body')}>
    <textarea
      id={ids.body}
      bind:value={form.body}
      onblur={() => fields.touch('body')}
      rows="10"
      placeholder={t('memory.editor.script.code.placeholder')}
      spellcheck="false"
      autocapitalize="off"
      aria-invalid={fields.messageOf('body') ? 'true' : undefined}
      class={fieldClass(fields.messageOf('body') !== null, 'resize-none px-3 py-2', 'font-mono text-sm')}
    ></textarea>
  </DocField>

  <!-- Code that hides characters has to be read with care: the editor says so, and shows the code the way a reviewer would see it. -->
  {#if hidden > 0}
    <div class="flex flex-col gap-2">
      <p class="rounded-md border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger" role="alert">{tn('memory.editor.script.hidden', hidden)}</p>
      <pre aria-label={t('memory.editor.script.hidden.preview')} class="max-h-48 overflow-auto rounded-md border border-line bg-canvas px-3 py-2 font-mono text-xs [overflow-wrap:anywhere] whitespace-pre-wrap text-fg"><code><HiddenText text={form.body} /></code></pre>
    </div>
  {/if}

  <DocField label={t('memory.editor.script.field.timeout')} for={ids.timeout} error={fields.messageOf('timeout')} help={t('memory.editor.script.timeout.help')}>
    <div class="flex items-center gap-2">
      <input
        id={ids.timeout}
        bind:value={form.timeout}
        onblur={() => fields.touch('timeout')}
        inputmode="numeric"
        autocomplete="off"
        placeholder={String(DEFAULT_TIMEOUT_SECS)}
        aria-invalid={fields.messageOf('timeout') ? 'true' : undefined}
        class={fieldClass(fields.messageOf('timeout') !== null, 'h-control w-28 px-3')}
      />
      <span class="text-sm text-fg-3">{t('memory.editor.script.timeout.unit')}</span>
    </div>
  </DocField>

  <EditorScope bind:value={form.target} projects={choices} id={ids.scope} error={fields.messageOf('scope')} help={scopeHelp} fixed={editing || pinned?.path === null} {place} />

  <EditorNotice level="info" icon={ShieldCheck} body={t(commit ? 'memory.guards.waiting.script.trust' : 'memory.editor.script.trust')} />
</EditorDialog>
