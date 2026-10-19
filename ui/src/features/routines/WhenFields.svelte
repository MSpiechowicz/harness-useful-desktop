<script lang="ts">
  import { slide } from 'svelte/transition';

  import DocField from '$lib/components/docs/DocField.svelte';
  import { fieldClass } from '$lib/components/docs/doc-field';
  import DocSelect from '$lib/components/docs/DocSelect.svelte';
  import { scaled } from '$lib/design/motion';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { SchedulePreview } from '$lib/ipc/bindings/SchedulePreview';
  import { IpcFailure } from '$lib/ipc/transport';
  import { uid } from '$lib/utils/ids';

  import { type FieldKey, type Problem, type RoutineForm, whenOf } from './routine-form';
  import { slotText, WHEN_HINTS, WHEN_ICONS, WHEN_KINDS, WHEN_LABELS } from './routine-format';
  import { routines } from './routines-data.svelte';

  interface Props {
    form: RoutineForm;
    problemOf: (field: FieldKey) => Problem | undefined;
    touch: (field: FieldKey) => void;
    /** Why the backend does not accept the schedule, null while it does or has not been asked. The editor does not save while there is one. */
    previewProblem: string | null;
  }

  /** How long the schedule waits for the next key before it is previewed, so typing a cron expression is one question. */
  const PREVIEW_DELAY_MS = 350;

  /** How many of the next runs the preview lists. */
  const SHOWN = 3;

  let { form = $bindable(), problemOf, touch, previewProblem = $bindable(null) }: Props = $props();

  const ids = { kind: uid('routine-when'), at: uid('routine-at'), every: uid('routine-every'), cron: uid('routine-cron') };

  let preview = $state<SchedulePreview | null>(null);
  let asked = 0;
  // The first schedule and a change of kind are previewed at once, so the line is there when the dialog opens. Only typing waits for the next key.
  let previewedKind: string | null = null;

  const options = $derived(WHEN_KINDS.map((kind) => ({ value: kind, label: t(WHEN_LABELS[kind]), hint: t(WHEN_HINTS[kind]), icon: WHEN_ICONS[kind] })));
  const hasEvery = $derived(form.whenKind === 'everyMinutes' || form.whenKind === 'everyHours');
  const hasAt = $derived(form.whenKind === 'daily' || form.whenKind === 'weekdays');

  // The next runs of the schedule as the backend reads it. A schedule that is not complete yet is not asked about.
  $effect(() => {
    const when = whenOf(form);

    asked += 1;
    const mine = asked;

    if (when === null || when.type === 'appStart') {
      preview = null;
      previewProblem = null;

      return;
    }

    const delay = previewedKind === when.type ? PREVIEW_DELAY_MS : 0;
    previewedKind = when.type;

    const timer = setTimeout(async () => {
      try {
        const answer = await routines.preview(when);

        if (mine === asked) {
          preview = answer;
          previewProblem = answer.problem;
        }
      } catch (error) {
        if (mine === asked) {
          preview = { next: [], problem: IpcFailure.from(error).message };
          previewProblem = null;
        }
      }
    }, delay);

    return () => clearTimeout(timer);
  });

  const everyUnit = $derived(tn(form.whenKind === 'everyHours' ? 'routines.editor.every.hours' : 'routines.editor.every.minutes', Number(form.every) || 0));
</script>

<DocField label={t('routines.editor.when')} labelId={ids.kind}>
  <DocSelect label={t('routines.editor.when')} {options} value={form.whenKind} labelledby={ids.kind} onchange={(kind) => (form.whenKind = kind)} />
</DocField>

{#if hasAt}
  {@const problem = problemOf('at')}

  <div transition:slide={{ duration: scaled(200) }} class="-m-1 overflow-hidden p-1">
    <DocField label={t('routines.editor.at')} for={ids.at} error={problem ? t(problem.message) : null}>
      <input id={ids.at} type="time" bind:value={form.at} onblur={() => touch('at')} aria-invalid={problem ? 'true' : undefined} class={fieldClass(problem !== undefined, 'h-control w-40 px-3')} />
    </DocField>
  </div>
{:else if hasEvery}
  {@const problem = problemOf('every')}

  <div transition:slide={{ duration: scaled(200) }} class="-m-1 overflow-hidden p-1">
    <DocField label={t('routines.editor.every')} for={ids.every} error={problem ? t(problem.message, problem.params) : null}>
      <div class="flex items-center gap-2">
        <input id={ids.every} bind:value={form.every} inputmode="numeric" autocomplete="off" aria-invalid={problem ? 'true' : undefined} class={fieldClass(problem !== undefined, 'h-control w-28 px-3')} />
        <span class="text-sm text-fg-3">{everyUnit}</span>
      </div>
    </DocField>
  </div>
{:else if form.whenKind === 'cron'}
  {@const problem = problemOf('cron')}

  <div transition:slide={{ duration: scaled(200) }} class="-m-1 overflow-hidden p-1">
    <DocField label={t('routines.editor.cron')} for={ids.cron} aside={t('routines.editor.cron.sub')} error={problem ? t(problem.message, problem.params) : previewProblem} help={t('routines.editor.cron.hint')}>
      <input
        id={ids.cron}
        bind:value={form.cron}
        onblur={() => touch('cron')}
        placeholder="0 9 * * 1-5"
        autocomplete="off"
        spellcheck="false"
        aria-invalid={problem || previewProblem ? 'true' : undefined}
        class={fieldClass(problem !== undefined || previewProblem !== null, 'h-control px-3', 'font-mono')}
      />
    </DocField>
  </div>
{/if}

{#if form.whenKind === 'appStart'}
  <p class="-mt-3 text-xs text-fg-3">{t('routines.editor.preview.appStart')}</p>
{:else}
  <!-- The line keeps its height while the next runs are asked, so the form below never jumps. -->
  <p class="-mt-3 min-h-4 text-xs" role="status">
    {#if preview?.problem && form.whenKind !== 'cron'}
      <span class="text-danger">{preview.problem}</span>
    {:else if preview && preview.next.length > 0}
      <span class="text-fg-3">{t('routines.editor.preview')}</span> <span class="text-fg-2">{preview.next.slice(0, SHOWN).map(slotText).join(' · ')}</span>
    {/if}
  </p>
{/if}
