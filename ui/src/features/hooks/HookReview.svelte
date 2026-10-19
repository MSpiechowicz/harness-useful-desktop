<script lang="ts">
  import { type DocReviewHandlers, type ReviewChange, reviewChanges } from '$lib/components/docs/doc-diff';
  import DocReview from '$lib/components/docs/DocReview.svelte';
  import { reviewTitleKey, type ReviewTitleKeys } from '$lib/components/docs/review-title';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MessageKey } from '$lib/i18n/locales/en';
  import type { HookView } from '$lib/ipc/bindings/HookView';

  import { lineDiff } from '../memory/line-diff';
  import { spelledOut } from '../memory/script-format';
  import ScriptDiff from '../memory/ScriptDiff.svelte';
  import { type ChangedField, changesOf } from './hook-diff';
  import { ACTION_LABELS, EVENT_LABELS, SCOPE_LABELS } from './hook-format';

  interface Props extends DocReviewHandlers {
    view: HookView;
  }

  let { view, busy, onapprove, onreject }: Props = $props();

  const hook = $derived(view.hook);
  // A hook that was never approved is new to the user; one with an approved version has been changed since.
  const added = $derived(view.approval === 'never');
  const byAgent = $derived(hook?.updatedBy === 'agent');
  const author = $derived(spelledOut(view.teamAuthor ?? t('hooks.review.team.someone')));
  const TITLES: ReviewTitleKeys = {
    added: 'hooks.review.added',
    addedOutside: 'hooks.review.addedOutside',
    changed: 'hooks.review.changed',
    changedOutside: 'hooks.review.changedOutside',
    teamAdded: 'hooks.review.team.added',
    teamChanged: 'hooks.review.team.changed',
  };

  const title = $derived(t(reviewTitleKey(TITLES, { team: view.origin === 'team', added, byAgent }), { author }));

  const LABELS: Record<Exclude<ChangedField, 'text'>, MessageKey> = {
    name: 'hooks.field.name',
    description: 'hooks.field.description',
    event: 'hooks.field.when',
    matcher: 'hooks.field.toolFilter',
    action: 'hooks.field.does',
    timeout: 'hooks.field.timeout',
    scope: 'hooks.field.appliesTo',
    project: 'hooks.field.project',
    enabled: 'hooks.field.enabled',
  };

  /** The heading of the text part names what it is: the context a hook adds, the message it sends or the command it runs. */
  const TEXT_LABELS = { addContext: 'hooks.field.context', followUp: 'hooks.field.message', command: 'hooks.field.command' } as const;

  const textLabel = $derived(t(TEXT_LABELS[hook?.action.type ?? 'command']));

  /** The parts that are code or a path, set in the mono face. */
  const MONO: ReadonlySet<ChangedField> = new Set(['matcher', 'project']);

  /** A value as a person reads it: the event by its name, a switch as on or off. */
  function shown(field: ChangedField, value: string | null): string | null {
    if (value === null) {
      return null;
    }

    switch (field) {
      case 'event':
        return t(EVENT_LABELS[value as keyof typeof EVENT_LABELS]);
      case 'action':
        return t(ACTION_LABELS[value as keyof typeof ACTION_LABELS]);
      case 'scope':
        return t(SCOPE_LABELS[value as keyof typeof SCOPE_LABELS]);
      case 'enabled':
        return t(value === 'true' ? 'hooks.value.on' : 'hooks.value.off');
      case 'timeout':
        return t('hooks.value.seconds', { count: value });
      case 'text':
        return value;
      default:
        // Names, descriptions and paths come from files an agent may have written: no character stays hidden.
        return spelledOut(value);
    }
  }

  const changes = $derived(
    reviewChanges(hook ? changesOf(hook, view.approved) : [], {
      label: (field) => (field === 'text' ? textLabel : t(LABELS[field])),
      value: shown,
      text: 'text',
      mono: MONO,
    }),
  );
</script>

{#snippet textDiff(change: ReviewChange)}
  <ScriptDiff diff={lineDiff(change.from ?? '', change.to ?? '')} label={t('hooks.review.diff', { field: change.label })} />
{/snippet}

<DocReview
  {title}
  body={t('hooks.review.body')}
  {changes}
  diffLabel={(field) => t('hooks.review.diff', { field })}
  {textDiff}
  approveLabel={t('hooks.review.approve')}
  rejectLabel={onreject ? t(added ? 'hooks.review.rejectRemove' : 'hooks.review.rejectUndo') : undefined}
  {busy}
  {onapprove}
  {onreject}
/>
