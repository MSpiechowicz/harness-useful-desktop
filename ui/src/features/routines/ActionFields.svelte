<script lang="ts">
  import { fade, slide } from 'svelte/transition';

  import DocField from '$lib/components/docs/DocField.svelte';
  import { fieldClass } from '$lib/components/docs/doc-field';
  import DocSelect from '$lib/components/docs/DocSelect.svelte';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import Switch from '$lib/components/Switch.svelte';
  import { scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { ApprovalMode } from '$lib/ipc/bindings/ApprovalMode';
  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
  import { ipc } from '$lib/ipc/client';
  import { harness as harnesses } from '$lib/stores/harness.svelte';
  import { uid } from '$lib/utils/ids';

  import { DEFAULT_MAX_MINUTES, DEFAULT_TIMEOUT_SECS, type FieldKey, LIMITS, length, type Problem, type RoutineForm, runsUnattended, withHarness } from './routine-form';
  import { abilitiesOf, offers, routineHarnesses } from './routine-abilities';
  import { ACTION_ICONS, ACTION_KINDS, ACTION_LABELS, type ActionKind, HARNESS_NAMES } from './routine-format';
  import { HarnessModels } from './harness-models.svelte';
  import ModelFields from './ModelFields.svelte';
  import ScriptPicker from './ScriptPicker.svelte';
  import UnattendedChip from './UnattendedChip.svelte';

  interface Props {
    form: RoutineForm;
    problemOf: (field: FieldKey) => Problem | undefined;
    touch: (field: FieldKey) => void;
  }

  let { form = $bindable(), problemOf, touch }: Props = $props();

  // The models and thinking levels asked of a harness, kept while the editor is open.
  const catalog = new HarnessModels(ipc);

  const ids = { kind: uid('routine-kind'), prompt: uid('routine-prompt'), harness: uid('routine-harness'), mode: uid('routine-mode'), max: uid('routine-max'), command: uid('routine-command'), timeout: uid('routine-timeout') };

  /** The approval modes a routine can ask for; `harness` leaves the choice to the harness's own setting. */
  const MODES: readonly (ApprovalMode | 'harness')[] = ['harness', 'alwaysAsk', 'write', 'auto', 'yolo'];
  const MODE_KEYS = {
    harness: ['routines.mode.harness', 'routines.mode.harness.hint'],
    alwaysAsk: ['routines.mode.alwaysAsk', 'routines.mode.alwaysAsk.hint'],
    write: ['routines.mode.write', 'routines.mode.write.hint'],
    auto: ['routines.mode.auto', 'routines.mode.auto.hint'],
    yolo: ['routines.mode.yolo', 'routines.mode.yolo.hint'],
  } as const;

  const kinds = $derived<StripOption<ActionKind>[]>(ACTION_KINDS.map((kind) => ({ value: kind, label: t(ACTION_LABELS[kind]), icon: ACTION_ICONS[kind] })));
  // A harness that is not installed here cannot start a chat, unless the routine already names it.
  const harnessOptions = $derived<StripOption<HarnessId>[]>(
    routineHarnesses(form.harness).map((id) => ({ value: id, label: HARNESS_NAMES[id], disabled: id !== form.harness && harnesses.get(id)?.detection.installed === false })),
  );
  const abilities = $derived(abilitiesOf(form.harness));
  const modes = $derived(MODES.filter((mode) => offers(abilities, mode)).map((mode) => ({ value: mode, label: t(MODE_KEYS[mode][0]), hint: t(MODE_KEYS[mode][1]) })));

  const prompt = $derived(problemOf('prompt'));
  const model = $derived(problemOf('model'));
  const command = $derived(problemOf('command'));
  const maxMinutes = $derived(problemOf('maxMinutes'));
  const timeout = $derived(problemOf('timeout'));
  const script = $derived(problemOf('script'));

  /** The characters used of a field's limit, said once the field is nearly full. */
  function counter(value: string, max: number): string | null {
    const used = length(value.trim());

    return used >= max * 0.75 ? t('routines.editor.counter', { used, max }) : null;
  }

  function message(problem: Problem | undefined): string | null {
    return problem ? t(problem.message, problem.params) : null;
  }
</script>

<DocField label={t('routines.editor.action')} labelId={ids.kind} help={t(`routines.editor.action.${form.kind}.hint`)}>
  <div class="flex"><SegmentedControl label={t('routines.editor.action')} options={kinds} bind:value={() => form.kind, (next) => (form.kind = next)} /></div>
</DocField>

{#if form.kind === 'agent'}
  <DocField label={t('routines.field.prompt')} for={ids.prompt} aside={counter(form.prompt, LIMITS.prompt) ?? t('routines.editor.prompt.sub')} error={message(prompt)}>
    <textarea
      id={ids.prompt}
      bind:value={form.prompt}
      onblur={() => touch('prompt')}
      rows="4"
      placeholder={t('routines.editor.prompt.placeholder')}
      aria-invalid={prompt ? 'true' : undefined}
      class={fieldClass(prompt !== undefined, 'resize-none px-3 py-2')}
    ></textarea>
  </DocField>

  <DocField label={t('routines.field.harness')} labelId={ids.harness}>
    <div class="flex"><SegmentedControl label={t('routines.field.harness')} options={harnessOptions} bind:value={() => form.harness, (next) => (form = withHarness(form, next))} /></div>
  </DocField>

  <ModelFields bind:form error={message(model)} {catalog} />

  <!-- What the harness lets a run do unattended: one slot, so a switch of harness swaps it in place instead of sliding one out and another in. -->
  <div class="grid">
    {#key form.harness}
      <div in:fade={{ duration: scaled(150), delay: scaled(60) }} out:fade={{ duration: scaled(90) }} class="flex flex-col gap-4 [grid-area:1/1]">
        {#if abilities.approvalMode}
          <DocField label={t('routines.field.approvalMode')} labelId={ids.mode}>
            <DocSelect label={t('routines.field.approvalMode')} options={modes} value={form.approvalMode ?? 'harness'} labelledby={ids.mode} onchange={(mode) => (form.approvalMode = mode === 'harness' ? null : mode)} />
          </DocField>
        {/if}
        {#if abilities.trustOnce}
          <div class="flex items-center justify-between gap-4">
            <div class="flex min-w-0 flex-col gap-0.5">
              <span class="text-sm text-fg-2">{t('routines.editor.trustOnce')}</span>
              <span class="text-xs text-fg-3">{t('routines.editor.trustOnce.hint', { harness: HARNESS_NAMES[form.harness] })}</span>
            </div>
            <Switch label={t('routines.editor.trustOnce')} bind:checked={form.trustOnce} />
          </div>
        {/if}
      </div>
    {/key}
  </div>

  {#if runsUnattended(form)}
    <div transition:slide={{ duration: scaled(200) }} class="-m-1 overflow-hidden p-1">
      <UnattendedChip hint={t('routines.editor.unattended')} />
    </div>
  {/if}

  <DocField label={t('routines.editor.maxMinutes.label')} for={ids.max} error={message(maxMinutes)} help={t('routines.editor.maxMinutes.hint', { default: DEFAULT_MAX_MINUTES, max: LIMITS.maxMinutes.max })}>
    <input id={ids.max} bind:value={form.maxMinutes} inputmode="numeric" autocomplete="off" placeholder={String(DEFAULT_MAX_MINUTES)} aria-invalid={maxMinutes ? 'true' : undefined} class={fieldClass(maxMinutes !== undefined, 'h-control px-3')} />
  </DocField>
{:else if form.kind === 'command'}
  <DocField label={t('routines.field.command')} for={ids.command} aside={counter(form.command, LIMITS.command) ?? t('routines.editor.command.sub')} error={message(command)} help={t('routines.editor.command.hint')}>
    <textarea
      id={ids.command}
      bind:value={form.command}
      onblur={() => touch('command')}
      rows="3"
      placeholder="bun run backup --quiet"
      spellcheck="false"
      aria-invalid={command ? 'true' : undefined}
      class={fieldClass(command !== undefined, 'resize-none px-3 py-2', 'font-mono text-sm')}
    ></textarea>
  </DocField>

  <DocField label={t('routines.editor.timeout.label')} for={ids.timeout} error={message(timeout)} help={t('routines.editor.timeout.hint', { default: DEFAULT_TIMEOUT_SECS, max: LIMITS.timeoutSecs.max })}>
    <input id={ids.timeout} bind:value={form.timeout} inputmode="numeric" autocomplete="off" placeholder={String(DEFAULT_TIMEOUT_SECS)} aria-invalid={timeout ? 'true' : undefined} class={fieldClass(timeout !== undefined, 'h-control px-3')} />
  </DocField>
{:else}
  <ScriptPicker bind:name={form.scriptName} bind:scope={form.scriptScope} routineScope={form.scope} project={form.projectPath} error={message(script)} onblur={() => touch('script')} />
{/if}
