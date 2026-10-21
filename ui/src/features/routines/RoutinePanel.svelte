<script lang="ts">
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import SquareArrowOutUpRight from '@lucide/svelte/icons/square-arrow-out-up-right';

  import Button from '$lib/components/Button.svelte';
  import { docScope, scopeText } from '$lib/components/docs/doc-scope';
  import DocInvalidNotice from '$lib/components/docs/DocInvalidNotice.svelte';
  import DocSwitchCell from '$lib/components/docs/DocSwitchCell.svelte';
  import { ICON } from '$lib/components/icon';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import FactsCard from '$lib/components/FactsCard.svelte';
  import SidePanel from '$lib/components/SidePanel.svelte';
  import SectionCard from '$lib/components/SectionCard.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MessageKey } from '$lib/i18n/locales/en';
  import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';
  import type { RoutineRun } from '$lib/ipc/bindings/RoutineRun';
  import type { RoutineView } from '$lib/ipc/bindings/RoutineView';
  import { cx } from '$lib/utils/cx';
  import { relativeTime } from '$lib/utils/format';

  import HiddenText from '../memory/HiddenText.svelte';
  import ScriptCode from '../memory/ScriptCode.svelte';
  import SettingRow from '../settings/SettingRow.svelte';
  import RecentRuns from './RecentRuns.svelte';
  import RoutineActions from './RoutineActions.svelte';
  import { routineFacts } from './routine-facts';
  import { actionText, isQueued, routineDoc, routineTitle, whenText } from './routine-format';
  import type { RoutineHandlers } from './routine-handlers';
  import { switchLock, teamSwitchedOn } from './routine-team';
  import RoutineReview from './RoutineReview.svelte';
  import UnattendedChip from './UnattendedChip.svelte';

  interface Props {
    open: boolean;
    /** Fills the column beside the list instead of sliding in over it. */
    inline: boolean;
    view: RoutineView | null;
    /** The projects on this computer, which the routine's scope is named by. */
    projects: ProjectScope[];
    /** The run going on now, if any. */
    running: RoutineRun | null;
    /** A change to this routine is being written. */
    busy: boolean;
    /** Routines are shared with the team: a local routine can be shared. */
    sharing: boolean;
    /** The local original of a team routine is still there, so its team copy can be updated from it. */
    canUpdate: boolean;
    /** A folder of the repository of a team routine is there on this computer (a project of it), which the routine is turned on and copied in. */
    projectKnown: boolean;
    handlers: RoutineHandlers;
  }

  let { open, inline, view, projects, running, busy, sharing, canUpdate, projectKnown, handlers }: Props = $props();

  const routine = $derived(view?.routine ?? null);
  const team = $derived(view?.origin === 'team');
  // Where the routine applies: Global, or the project it belongs to, which is not always the open one.
  const scope = $derived(view ? scopeText(docScope(routineDoc(view), projects)) : '');
  const info = NOTICE_LOOKS.info;
  const warning = NOTICE_LOOKS.warning;
  const waiting = $derived(running?.needsYou === true);

  const STATUS_TEXT: Record<string, MessageKey> = {
    active: 'routines.panel.status.active',
    running: 'routines.panel.status.running',
    off: 'routines.panel.status.off',
    pending: 'routines.panel.status.pending',
    paused: 'routines.panel.status.paused',
    invalid: 'routines.panel.status.invalid',
    duplicate: 'routines.panel.status.duplicate',
  };

  const TEXT_HEADINGS = { agent: 'routines.field.prompt', command: 'routines.field.command', script: 'routines.field.script' } as const;

  // Why the switch and Edit are off: both would write the file, and a write approves what it writes. A team routine has no Edit.
  const lockReason = $derived.by(() => {
    const key = view ? switchLock(view, projectKnown) : null;

    return key === null ? null : t(key);
  });

  // The status line of a team routine that runs or not by this computer's choice says whose choice the switch is.
  const statusText = $derived.by(() => {
    if (!view || !routine) {
      return '';
    }

    const text = t(STATUS_TEXT[view.status], { when: whenText(routine.when) });

    if (isQueued(view) && !running) {
      return t('routines.panel.status.queued');
    }

    return team && (view.status === 'active' || view.status === 'off') ? `${text} ${t('routines.panel.team.onlyHere')}` : text;
  });
</script>

<SidePanel
  {open}
  {inline}
  title={view ? routineTitle(view) : ''}
  subtitle={routine ? `${whenText(routine.when)} · ${scope}${team ? ` · ${t('routines.source.team')}` : ''}` : (view?.id ?? '')}
  onclose={handlers.close}
>
  {#if view}
    <SectionCard flat heading={t('routines.panel.status')}>
      {#if routine}
        <div class="-my-4">
          <SettingRow title={view.status === 'duplicate' ? t('routines.status.duplicate') : t('routines.panel.enabled')} description={statusText}>
            {#snippet control()}
              <DocSwitchCell
                label={t(team ? 'routines.panel.team.enabledLabel' : 'routines.panel.enabledLabel', { name: routineTitle(view) })}
                checked={team ? teamSwitchedOn(view) : view.status !== 'pending' && routine.enabled}
                {lockReason}
                {busy}
                onchange={handlers.toggle}
              />
            {/snippet}
          </SettingRow>
        </div>
      {/if}

      <!-- A paused routine says what is missing, as a team routine does: the folder it runs in. -->
      {#if view.status === 'paused' && view.problem}
        <div class={cx('flex items-start gap-3 rounded-md p-3', warning.box)} role="status">
          <span class="flex h-5 shrink-0 items-center"><warning.icon class={warning.color} size={ICON.sm} aria-hidden="true" /></span>
          <p class="min-w-0 text-sm break-words text-fg-2">{view.problem}</p>
        </div>
      {/if}

      {#if view.status === 'invalid'}
        <DocInvalidNotice title={t('routines.panel.invalid.title')} problem={view.problem ?? t('routines.panel.invalid.unknown')} file="{view.id}.json" />
      {/if}
    </SectionCard>

    {#if running}
      <SectionCard flat heading={t('routines.panel.running')}>
        <div class={cx('flex flex-col gap-3 rounded-md p-3', waiting ? warning.box : info.box)} role="status">
          <p class="text-sm font-medium text-fg">{t(waiting ? 'routines.panel.waiting.title' : 'routines.panel.running.title', { when: relativeTime(running.startedAt) })}</p>
          {#if waiting}<p class="text-sm text-fg-2">{t('routines.panel.waiting.body')}</p>{/if}

          <div class="flex flex-wrap items-center gap-2">
            {#if waiting}
              <Button variant="primary" onclick={handlers.openChat}>
                {#snippet leading()}<MessageSquare size={ICON.sm} aria-hidden="true" />{/snippet}
                {t('routines.panel.openChat')}
              </Button>
            {/if}
            {#if running.session}
              <Button onclick={handlers.openSession}>
                {#snippet leading()}<SquareArrowOutUpRight size={ICON.sm} aria-hidden="true" />{/snippet}
                {t('routines.panel.openSession')}
              </Button>
            {/if}
          </div>
        </div>
      </SectionCard>
    {/if}

    {#if view.status === 'pending'}
      <SectionCard flat heading={t('routines.panel.review')}>
        <RoutineReview {view} {projects} {busy} onapprove={handlers.approve} onreject={team ? undefined : handlers.reject} />
      </SectionCard>
    {/if}

    {#if routine}
      {#if routine.description}
        <SectionCard flat heading={t('routines.field.description')}>
          <p class="text-sm leading-relaxed break-words text-fg-2"><HiddenText text={routine.description} /></p>
        </SectionCard>
      {/if}

      <FactsCard flat heading={t('routines.panel.details')} rows={routineFacts(routine, view.approval, view, scope)} />

      <SectionCard flat heading={t(TEXT_HEADINGS[routine.action.type])}>
        {#if routine.action.type === 'command'}
          <ScriptCode body={routine.action.command} language="sh" />
        {:else if routine.action.type === 'script'}
          <p class="font-mono text-sm break-all text-fg"><HiddenText text={routine.action.name} /></p>
        {:else}
          <p class="chat-radius border border-line bg-canvas px-3.5 py-3 text-sm leading-relaxed break-words whitespace-pre-wrap text-fg"><HiddenText text={actionText(routine.action)} /></p>
        {/if}

        {#if view.runsUnattended}<div class="mt-3"><UnattendedChip hint={t('routines.panel.unattended')} /></div>{/if}
      </SectionCard>

      <SectionCard flat heading={t('routines.panel.runs')}>
        <RecentRuns routineId={view.id} onopen={(runId) => handlers.showRuns(runId)} onall={() => handlers.showRuns()} />
      </SectionCard>
    {/if}

    <SectionCard flat heading={t('routines.panel.actions')}>
      <RoutineActions
        {view}
        {lockReason}
        {sharing}
        {canUpdate}
        {projectKnown}
        running={running !== null}
        {busy}
        onrun={handlers.run}
        onstop={handlers.stop}
        onedit={handlers.edit}
        ondelete={handlers.remove}
        onshare={handlers.share}
        onupdate={handlers.update}
        onremove={handlers.removeTeam}
        oncopy={handlers.copy}
      />
    </SectionCard>
  {/if}
</SidePanel>
