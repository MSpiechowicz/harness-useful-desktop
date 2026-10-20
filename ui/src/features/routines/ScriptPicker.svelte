<script lang="ts">
  import FileCode from '@lucide/svelte/icons/file-code';
  import { untrack } from 'svelte';

  import DocField from '$lib/components/docs/DocField.svelte';
  import { fieldClass } from '$lib/components/docs/doc-field';
  import DocSelect from '$lib/components/docs/DocSelect.svelte';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HookScope } from '$lib/ipc/bindings/HookScope';
  import type { MemoryScriptsPage } from '$lib/ipc/bindings/MemoryScriptsPage';
  import type { ScriptScope } from '$lib/ipc/bindings/ScriptScope';
  import { ipc } from '$lib/ipc/client';
  import { ViewLoader } from '$lib/stores/view-loader.svelte';
  import { uid } from '$lib/utils/ids';

  import { languageLabel, needsReview } from '../memory/script-format';

  interface Props {
    /** The script's name and the scope Harness Memory keeps it in. */
    name: string;
    scope: ScriptScope;
    /** The routine's own scope: a global routine has no repository to look a script up in. */
    routineScope: HookScope;
    /** The folder of the project the routine is for, which the scripts of its repository are listed for; null for a global routine. */
    project: string | null;
    /** What is wrong with the choice, if the editor says so. */
    error: string | null;
    onblur: () => void;
  }

  /** The most scripts the picker lists. The rest are typed. */
  const LIMIT = 200;

  let { name = $bindable(), scope = $bindable(), routineScope, project, error, onblur }: Props = $props();

  const ids = { picker: uid('routine-script'), name: uid('routine-script-name') };

  const saved = new ViewLoader<MemoryScriptsPage, string>((_key, path) => ipc.invoke('memory_scripts_list', { project: path, offset: 0, limit: LIMIT, sort: 'name', order: 'asc' }));

  $effect(() => {
    const where = project;

    untrack(() => saved.sync('scripts', where));
  });

  const page = $derived(saved.data);
  const listed = $derived(page && page.availability.kind === 'available' ? page.scripts.filter((script) => routineScope === 'project' || script.scope === 'global') : []);
  const current = $derived(listed.find((script) => script.name === name && script.scope === scope));
  const choices = $derived([
    ...listed.map((script) => ({
      value: `${script.scope}:${script.name}`,
      label: script.name,
      hint: [languageLabel(script.language), t(script.scope === 'repo' ? 'routines.value.scriptRepo' : 'routines.value.scriptGlobal'), needsReview(script) ? t('routines.editor.script.needsReview') : script.description].filter(Boolean).join(' · '),
      icon: FileCode,
    })),
    // A script the routine names that is not (or no longer) among the saved ones stays chosen, so that editing does not drop it silently.
    ...(name !== '' && !current ? [{ value: `${scope}:${name}`, label: name, hint: t('routines.editor.script.missing'), icon: FileCode }] : []),
  ]);
  // Without a list to pick from (Harness Memory is not there or has none), the name is typed.
  const typed = $derived(page === null || listed.length === 0);
  const scopes = $derived<StripOption<ScriptScope>[]>([
    { value: 'global', label: t('routines.value.scriptGlobal') },
    { value: 'repo', label: t('routines.value.scriptRepo'), disabled: routineScope === 'global' },
  ]);

  function choose(key: string): void {
    const at = key.indexOf(':');

    scope = key.slice(0, at) as ScriptScope;
    name = key.slice(at + 1);
    onblur();
  }
</script>

{#if typed}
  <DocField label={t('routines.editor.script')} for={ids.name} {error} help={page ? t('routines.editor.script.none') : undefined}>
    <input id={ids.name} bind:value={name} {onblur} placeholder="nightly-backup" autocomplete="off" spellcheck="false" aria-invalid={error ? 'true' : undefined} class={fieldClass(error !== null, 'h-control px-3', 'font-mono')} />
  </DocField>

  <DocField label={t('routines.editor.script.scope')} labelId={ids.picker}>
    <div class="flex"><SegmentedControl label={t('routines.editor.script.scope')} options={scopes} bind:value={scope} /></div>
  </DocField>
{:else}
  <DocField label={t('routines.editor.script')} labelId={ids.picker} {error} help={current && needsReview(current) ? t('routines.editor.script.review') : t('routines.editor.script.hint')}>
    <DocSelect label={t('routines.editor.script')} options={choices} value={name === '' ? '' : `${scope}:${name}`} labelledby={ids.picker} onchange={choose} placeholder={t('routines.editor.script.choose')} />
  </DocField>
{/if}
