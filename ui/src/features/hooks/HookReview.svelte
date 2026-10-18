<script lang="ts">
  import ShieldCheck from '@lucide/svelte/icons/shield-check';

  import Button from '$lib/components/Button.svelte';
  import { DIFF_ROW, DIFF_TEXT } from '$lib/components/diff-tones';
  import { ICON } from '$lib/components/icon';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MessageKey } from '$lib/i18n/locales/en';
  import type { HookView } from '$lib/ipc/bindings/HookView';
  import { cx } from '$lib/utils/cx';

  import { lineDiff } from '../memory/line-diff';
  import { spelledOut } from '../memory/script-format';
  import ScriptDiff from '../memory/ScriptDiff.svelte';
  import { type ChangedField, changesOf, type FieldChange } from './hook-diff';
  import { ACTION_LABELS, EVENT_LABELS, SCOPE_LABELS } from './hook-format';

  interface Props {
    view: HookView;
    /** The approve or reject is being written. */
    busy: boolean;
    onapprove: () => void;
    /** Left out for a team hook, which the user can only approve or leave: the team's copy is not theirs to undo. */
    onreject?: () => void;
  }

  let { view, busy, onapprove, onreject }: Props = $props();

  const look = NOTICE_LOOKS.warning;
  const hook = $derived(view.hook);
  const changes = $derived(hook ? changesOf(hook, view.approved) : []);
  // A hook that was never approved is new to the user; one with an approved version has been changed since.
  const added = $derived(view.approval === 'never');
  const byAgent = $derived(hook?.updatedBy === 'agent');
  const author = $derived(spelledOut(view.teamAuthor ?? t('hooks.review.team.someone')));
  const title = $derived.by(() => {
    if (view.origin === 'team') {
      return t(added ? 'hooks.review.team.added' : 'hooks.review.team.changed', { author });
    }

    return t(added ? (byAgent ? 'hooks.review.added' : 'hooks.review.addedOutside') : byAgent ? 'hooks.review.changed' : 'hooks.review.changedOutside');
  });

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

  function label(change: FieldChange): string {
    return change.field === 'text' ? textLabel : t(LABELS[change.field]);
  }

  /** A value as a person reads it: the event by its name, a switch as on or off. */
  function shown(field: ChangedField, value: string): string {
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
      default:
        // Names, descriptions and paths come from files an agent may have written: no character stays hidden.
        return spelledOut(value);
    }
  }
</script>

<!-- Nothing an agent wrote runs before this: the user reads exactly what is new, and approves that version or sends it away. -->
<div class={cx('flex flex-col gap-3 rounded-md p-3', look.box)} role="status">
  <div class="flex min-w-0 items-start gap-3">
    <span class="flex h-5 shrink-0 items-center"><look.icon class={look.color} size={ICON.sm} aria-hidden="true" /></span>
    <div class="flex min-w-0 flex-col gap-0.5">
      <p class="text-sm font-medium text-fg">{title}</p>
      <p class="text-sm text-fg-2">{t('hooks.review.body')}</p>
    </div>
  </div>

  <div class="flex flex-col gap-3 rounded-md border border-line bg-surface-1 p-3">
    {#each changes as change (change.field)}
      <div class="flex flex-col gap-1.5">
        <p class="text-xs font-medium tracking-wider text-fg-3 uppercase">{label(change)}</p>

        {#if change.field === 'text'}
          <ScriptDiff diff={lineDiff(change.from ?? '', change.to ?? '')} label={t('hooks.review.diff', { field: label(change) })} />
        {:else}
          <div class="chat-radius overflow-hidden border border-line bg-canvas py-1 text-sm leading-5 break-words" role="group" aria-label={t('hooks.review.diff', { field: label(change) })}>
            {#if change.from !== null}
              <p class={cx('grid grid-cols-[1.5ch_1fr] gap-x-2 px-3.5', DIFF_ROW.remove, DIFF_TEXT.remove, MONO.has(change.field) && 'font-mono')}><span aria-hidden="true">−</span><span>{shown(change.field, change.from)}</span></p>
            {/if}
            {#if change.to !== null}
              <p class={cx('grid grid-cols-[1.5ch_1fr] gap-x-2 px-3.5', DIFF_ROW.add, DIFF_TEXT.add, MONO.has(change.field) && 'font-mono')}><span aria-hidden="true">+</span><span>{shown(change.field, change.to)}</span></p>
            {/if}
          </div>
        {/if}
      </div>
    {/each}
  </div>

  <div class="flex flex-wrap items-center gap-2">
    <Button variant="primary" disabled={busy} onclick={onapprove}>
      {#snippet leading()}<ShieldCheck size={ICON.sm} aria-hidden="true" />{/snippet}
      {t('hooks.review.approve')}
    </Button>
    {#if onreject}
      <Button variant="dangerQuiet" disabled={busy} onclick={onreject}>{t(added ? 'hooks.review.rejectRemove' : 'hooks.review.rejectUndo')}</Button>
    {/if}
  </div>
</div>
