<script lang="ts">
  import ShieldCheck from '@lucide/svelte/icons/shield-check';
  import { untrack } from 'svelte';

  import DocField from '$lib/components/docs/DocField.svelte';
  import { fieldClass } from '$lib/components/docs/doc-field';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { ScriptDetail } from '$lib/ipc/bindings/ScriptDetail';
  import type { ScriptLanguage } from '$lib/ipc/bindings/ScriptLanguage';
  import type { ScriptSaved } from '$lib/ipc/bindings/ScriptSaved';
  import { rereadScript, SCRIPT_CHANGED, saveScript } from '$lib/stores/memoryScripts.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { uid } from '$lib/utils/ids';

  import { fieldProblems, onOpen } from './editor-fields.svelte';
  import EditorDialog from './EditorDialog.svelte';
  import EditorNotice from './EditorNotice.svelte';
  import ExistsNotice from './ExistsNotice.svelte';
  import HiddenText from './HiddenText.svelte';
  import ScopeField from './ScopeField.svelte';
  import ScriptChangedNotice from './ScriptChangedNotice.svelte';
  import { blankForm, codeChanged, DEFAULT_TIMEOUT_SECS, draftOf, type FieldKey, formOf, LANGUAGES, problemsOf, type ScriptForm, type ScriptScope, unsupportedLanguage } from './script-form';
  import { languageLabel, needsReview, revealHidden } from './script-format';

  interface Props {
    open: boolean;
    /** The script to edit, with its code; null adds a new one. */
    script: ScriptDetail | null;
    /** The script was written; the editor is closing. `codeChanged` tells whether the code differs from what the editor was opened on. */
    onsaved: (saved: ScriptSaved, name: string, codeChanged: boolean) => void;
    /** The user chose to open the script that has the name instead of saving. The editor is closing. */
    onopen: (id: number) => void;
  }

  let { open = $bindable(false), script, onsaved, onopen }: Props = $props();

  const FIELDS: readonly FieldKey[] = ['name', 'description', 'body', 'timeout', 'scope'];

  const ids = { name: uid('script-name'), description: uid('script-description'), language: uid('script-language'), body: uid('script-body'), timeout: uid('script-timeout'), scope: uid('script-scope') };

  /** What a save found: this script (of this name, in this scope) already exists. */
  interface Taken {
    id: number;
    name: string;
    scope: ScriptScope;
  }

  let form = $state<ScriptForm>(blankForm(false));
  let saving = $state(false);
  let taken = $state<Taken | null>(null);
  /** The version of the script the form was filled from: the one opened, or the one the user reloaded. Its hash is what an edit is saved against. */
  let opened = $state.raw<ScriptDetail | null>(null);
  /** The last save found the script changed (`changed`) or deleted (`gone`) since `opened` was read. */
  let stale = $state<'changed' | 'gone' | null>(null);
  let reloading = $state(false);

  const editing = $derived(script !== null);
  // A script that waits for review stays waiting unless its code is changed here, which makes it the user's own.
  const waiting = $derived(opened !== null && needsReview(opened));
  // A language this version does not know is not edited: saving would have to name another one.
  const unsupported = $derived(opened ? unsupportedLanguage(opened) : null);
  const projectPath = $derived(project.active?.path ?? null);

  // The editor opens on what it was given, fresh each time.
  onOpen(
    () => open,
    () => {
      opened = script;
      form = script ? formOf(script) : blankForm(untrack(() => projectPath) !== null);
      taken = null;
      stale = null;
    },
  );

  const fields = fieldProblems(() => problemsOf(form, { projectOpen: projectPath !== null, editing }), () => open);

  // The finding stands only while the draft is still the one it was found for.
  const conflict = $derived(taken && taken.name === form.name.trim() && taken.scope === form.scope ? taken : null);
  const hidden = $derived(revealHidden(form.body).count);
  const languages = $derived<StripOption<ScriptLanguage>[]>(LANGUAGES.map((language) => ({ value: language, label: languageLabel(language) })));
  const scopeHelp = $derived.by(() => {
    if (editing) {
      return t('memory.editor.script.scope.fixed');
    }

    return form.scope === 'repo' ? t('memory.editor.script.scope.repo.hint', { project: project.active?.name ?? '' }) : t('memory.editor.script.scope.global.hint');
  });

  /** Writes the script. `replace` overwrites the script that has the name: an edit, or the user's choice on a taken name. */
  async function submit(replace = editing): Promise<void> {
    fields.touchAll(FIELDS);

    if (fields.blocked || saving || unsupported !== null || stale === 'gone') {
      return;
    }

    saving = true;

    try {
      const draft = draftOf(form);
      // An edit is saved against the version it opened, never an add, not even one that replaces the script of that name.
      const saved = await saveScript(projectPath, draft, replace, editing ? opened?.sha256 : undefined);

      if (saved === null) {
        return;
      }

      if (saved === SCRIPT_CHANGED) {
        stale = 'changed';

        return;
      }

      if (saved.outcome === 'exists') {
        taken = { id: saved.id, name: draft.name, scope: draft.scope };

        return;
      }

      open = false;
      onsaved(saved, draft.name, opened === null || codeChanged(opened.body, draft.body));
    } finally {
      saving = false;
    }
  }

  /** Reads the script again and fills the form with it: the edits made on the old version are dropped. */
  async function reload(): Promise<void> {
    if (opened === null || reloading) {
      return;
    }

    reloading = true;

    try {
      const latest = await rereadScript(opened.id, viewState.memoryReach);

      if (latest === 'gone') {
        stale = 'gone';
      } else if (latest !== null) {
        opened = latest;
        form = formOf(latest);
        stale = null;
      }
    } finally {
      reloading = false;
    }
  }

  function openTaken(id: number): void {
    open = false;
    onopen(id);
  }
</script>

<!-- The focus starts on the first field that can be typed in: the name of a new script, the description of an edited one. -->
<EditorDialog
  bind:open
  title={t(editing ? 'memory.editor.script.title.edit' : 'memory.editor.script.title.new')}
  description={t(editing ? 'memory.editor.script.description.edit' : 'memory.editor.script.description.new')}
  initialFocus={`#${editing ? ids.description : ids.name}`}
  saveLabel={t(editing ? 'memory.editor.script.save' : 'memory.editor.script.add')}
  {saving}
  blocked={fields.blocked || unsupported !== null || stale === 'gone'}
  onsubmit={() => void submit()}
>
  {#if stale !== null}
    <ScriptChangedNotice gone={stale === 'gone'} {reloading} onreload={() => void reload()} onclose={() => (open = false)} />
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

  <ScopeField bind:value={form.scope} fixed={editing} projectOpen={projectPath !== null} id={ids.scope} error={fields.messageOf('scope')} help={scopeHelp} />

  <EditorNotice level="info" icon={ShieldCheck} body={t('memory.editor.script.trust')} />
</EditorDialog>
