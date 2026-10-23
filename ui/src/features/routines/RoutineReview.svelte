<script lang="ts">
  import { type DocReviewHandlers, type ReviewChange, reviewChanges } from '$lib/components/docs/doc-diff';
  import DocReview from '$lib/components/docs/DocReview.svelte';
  import { reviewTitleKey, type ReviewTitleKeys } from '$lib/components/docs/review-title';
  import { versionScopeText } from '$lib/components/docs/doc-scope';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MessageKey } from '$lib/i18n/locales/en';
  import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';
  import type { RoutineView } from '$lib/ipc/bindings/RoutineView';

  import { lineDiff } from '../memory/line-diff';
  import { spelledOut } from '../memory/script-format';
  import ScriptDiff from '../memory/ScriptDiff.svelte';
  import { type ChangedField, changesOf } from './routine-diff';
  import { routineDoc } from './routine-format';
  import UnattendedChip from './UnattendedChip.svelte';

  interface Props extends DocReviewHandlers {
    view: RoutineView;
    /** The projects on this computer, which a routine's scope is named by. */
    projects: ProjectScope[];
  }

  let { view, projects, busy, onapprove, onreject }: Props = $props();

  const routine = $derived(view.routine);
  // A routine that was never approved is new to the user. One with an approved version has been changed since.
  const added = $derived(view.approval === 'never');
  const byAgent = $derived(routine?.updatedBy === 'agent');
  const author = $derived(spelledOut(view.teamAuthor ?? t('routines.review.team.someone')));
  const TITLES: ReviewTitleKeys = {
    added: 'routines.review.added',
    addedOutside: 'routines.review.addedOutside',
    changed: 'routines.review.changed',
    changedOutside: 'routines.review.changedOutside',
    teamAdded: 'routines.review.team.added',
    teamChanged: 'routines.review.team.changed',
  };

  const title = $derived(t(reviewTitleKey(TITLES, { team: view.origin === 'team', added, byAgent }), { author }));

  const LABELS: Record<Exclude<ChangedField, 'text'>, MessageKey> = {
    name: 'routines.field.name',
    description: 'routines.field.description',
    when: 'routines.field.when',
    does: 'routines.field.does',
    harness: 'routines.field.harness',
    model: 'routines.field.model',
    thinking: 'routines.field.thinking',
    approval: 'routines.field.approval',
    maxMinutes: 'routines.field.maxMinutes',
    timeout: 'routines.field.timeout',
    scriptScope: 'routines.field.scriptScope',
    scope: 'routines.field.appliesTo',
    project: 'routines.field.project',
    enabled: 'routines.field.enabled',
  };

  /** The heading of the text part names what it is: the prompt it sends, the command it runs or the script it starts. */
  const TEXT_LABELS = { agent: 'routines.field.prompt', command: 'routines.field.command', script: 'routines.field.script' } as const;

  const textLabel = $derived(t(TEXT_LABELS[routine?.action.type ?? 'agent']));

  /** The parts that are code or a path, set in the mono face. */
  const MONO: ReadonlySet<ChangedField> = new Set(['model', 'project']);

  /** Names, descriptions and paths come from files an agent may have written: no character stays hidden. */
  const SPELLED: ReadonlySet<ChangedField> = new Set(['name', 'description', 'scope', 'project']);

  function shown(field: ChangedField, value: string | null): string | null {
    return value !== null && SPELLED.has(field) ? spelledOut(value) : value;
  }

  const changes = $derived(
    reviewChanges(routine ? changesOf(routine, view.approved, (version) => versionScopeText(routineDoc(view), version, projects)) : [], {
      mono: MONO,
      text: 'text',
      value: shown,
      label: (field) => (field === 'text' ? textLabel : t(LABELS[field])),
    }),
  );
</script>

{#snippet textDiff(change: ReviewChange)}
  <ScriptDiff diff={lineDiff(change.from ?? '', change.to ?? '')} label={t('routines.review.diff', { field: change.label })} />
{/snippet}

{#snippet extra()}
  {#if view.runsUnattended}
    <UnattendedChip hint={t('routines.review.unattended')} />
  {/if}
{/snippet}

<DocReview
  {title}
  body={t('routines.review.body')}
  {changes}
  diffLabel={(field) => t('routines.review.diff', { field })}
  {textDiff}
  {extra}
  approveLabel={t('routines.review.approve')}
  rejectLabel={onreject ? t(added ? 'routines.review.rejectRemove' : 'routines.review.rejectUndo') : undefined}
  {busy}
  {onapprove}
  {onreject}
/>
