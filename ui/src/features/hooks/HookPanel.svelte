<script lang="ts">
  import Pause from '@lucide/svelte/icons/pause';

  import Button from '$lib/components/Button.svelte';
  import { docScope, scopeText } from '$lib/components/docs/doc-scope';
  import DocSwitchCell from '$lib/components/docs/DocSwitchCell.svelte';
  import DocInvalidNotice from '$lib/components/docs/DocInvalidNotice.svelte';
  import FactsCard from '$lib/components/FactsCard.svelte';
  import { ICON } from '$lib/components/icon';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import SectionCard from '$lib/components/SectionCard.svelte';
  import SidePanel from '$lib/components/SidePanel.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MessageKey } from '$lib/i18n/locales/en';
  import type { HookView } from '$lib/ipc/bindings/HookView';
  import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';
  import { nav } from '$lib/stores/nav.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { cx } from '$lib/utils/cx';

  import HiddenText from '../memory/HiddenText.svelte';
  import ScriptCode from '../memory/ScriptCode.svelte';
  import SettingRow from '../settings/SettingRow.svelte';
  import FamilyInstall from './FamilyInstall.svelte';
  import { hookFacts } from './hook-facts';
  import { actionText, EVENT_LABELS, EVENT_WHEN, hookDoc, hookTitle, MEMBER_LABELS, waitingFor } from './hook-format';
  import { switchLock, teamSwitchedOn } from './hook-team';
  import HookActions from './HookActions.svelte';
  import HookReview from './HookReview.svelte';

  interface Props {
    open: boolean;
    /** In a column of its own beside the list, not sliding in over it. */
    inline: boolean;
    view: HookView | null;
    /** The projects on this computer, which the hook's scope is named by. */
    projects: ProjectScope[];
    /** A change to this hook is being written. */
    busy: boolean;
    /** Hooks are shared with the team: a local hook can be shared. */
    sharing: boolean;
    /** The local original of a team hook is still there, so its team copy can be updated from it. */
    canUpdate: boolean;
    /** Why a team hook cannot be copied to the user's hooks (its repository is not on this computer), or null. */
    copyLock: string | null;
    ontoggle: (enabled: boolean) => void;
    onapprove: () => void;
    onreject: () => void;
    onedit: () => void;
    ondelete: () => void;
    onshare: () => void;
    onupdate: () => void;
    onremove: () => void;
    oncopy: () => void;
    onclose: () => void;
  }

  let { open, inline, view, projects, busy, sharing, canUpdate, copyLock, ontoggle, onapprove, onreject, onedit, ondelete, onshare, onupdate, onremove, oncopy, onclose }: Props = $props();

  const hook = $derived(view?.hook ?? null);
  const info = NOTICE_LOOKS.info;

  const STATUS_TEXT: Record<string, MessageKey> = {
    active: 'hooks.panel.status.active',
    off: 'hooks.panel.status.off',
    pending: 'hooks.panel.status.pending',
    paused: 'hooks.panel.status.paused',
    invalid: 'hooks.panel.status.invalid',
    duplicate: 'hooks.panel.status.duplicate',
  };

  const TEXT_HEADINGS = { addContext: 'hooks.field.context', followUp: 'hooks.field.message', command: 'hooks.field.command' } as const;

  const waiting = $derived(hook && view?.status === 'paused' ? waitingFor(hook, prefs.value?.memoryEnabled !== false) : null);
  const team = $derived(view?.origin === 'team');
  // Where the hook applies: Global, or the project it belongs to, which is not always the open one.
  const scope = $derived(view ? scopeText(docScope(hookDoc(view), projects)) : '');
  // Why the switch and Edit are off: both would write the file, and a write approves what it writes. A team hook has no Edit.
  const lockReason = $derived.by(() => {
    const key = view ? switchLock(view) : null;

    return key === null ? null : t(key);
  });
  // The status line of a team hook that runs or not by this computer's choice says whose choice the switch is.
  const statusText = $derived.by(() => {
    if (!view || !hook) {
      return '';
    }

    const text = t(STATUS_TEXT[view.status], { when: t(EVENT_WHEN[hook.event]) });

    return team && (view.status === 'active' || view.status === 'off') ? `${text} ${t('hooks.panel.team.onlyHere')}` : text;
  });

  function openSettings(): void {
    viewState.settingsTarget = 'memory';
    nav.go('settings');
  }
</script>

<SidePanel
  {open}
  {inline}
  title={view ? hookTitle(view) : ''}
  subtitle={hook ? `${t(EVENT_LABELS[hook.event])} · ${scope}${team ? ` · ${t('hooks.source.team')}` : ''}` : (view?.id ?? '')}
  {onclose}
>
  {#if view}
    <SectionCard flat heading={t('hooks.panel.status')}>
      {#if hook}
        <div class="-my-4">
          <SettingRow title={view.status === 'duplicate' ? t('hooks.status.duplicate') : t('hooks.panel.enabled')} description={statusText}>
            {#snippet control()}
              <DocSwitchCell
                label={t(team ? 'hooks.panel.team.enabledLabel' : 'hooks.panel.enabledLabel', { name: hookTitle(view) })}
                checked={team ? teamSwitchedOn(view) : view.status !== 'pending' && hook.enabled}
                {lockReason}
                {busy}
                onchange={ontoggle}
              />
            {/snippet}
          </SettingRow>
        </div>
      {/if}

      {#if view.status === 'invalid'}
        <DocInvalidNotice title={t('hooks.panel.invalid.title')} problem={view.problem ?? t('hooks.panel.invalid.unknown')} file="{view.id}.json" />
      {:else if waiting}
        <div class={cx('mt-3 flex flex-col gap-3 rounded-md p-3', info.box)} role="status">
          <div class="flex min-w-0 items-start gap-3">
            <span class="flex h-5 shrink-0 items-center"><Pause class={info.color} size={ICON.sm} aria-hidden="true" /></span>
            <div class="flex min-w-0 flex-col gap-0.5">
              <p class="text-sm font-medium text-fg">{t(waiting.off ? 'hooks.waiting.off.title' : 'hooks.waiting.missing.title', { member: t(MEMBER_LABELS[waiting.member]) })}</p>
              <p class="text-sm text-fg-2">{t(waiting.off ? 'hooks.waiting.off.body' : 'hooks.waiting.missing.body')}</p>
            </div>
          </div>

          {#if waiting.off}
            <Button onclick={openSettings}>{t('hooks.waiting.openSettings')}</Button>
          {:else}
            <FamilyInstall member={waiting.member} />
          {/if}
        </div>
      {/if}
    </SectionCard>

    {#if view.status === 'pending'}
      <SectionCard flat heading={t('hooks.panel.review')}>
        <HookReview {view} {projects} {busy} {onapprove} onreject={team ? undefined : onreject} />
      </SectionCard>
    {/if}

    {#if hook}
      {#if hook.description}
        <SectionCard flat heading={t('hooks.field.description')}>
          <p class="text-sm leading-relaxed break-words text-fg-2"><HiddenText text={hook.description} /></p>
        </SectionCard>
      {/if}

      <FactsCard flat heading={t('hooks.panel.details')} rows={hookFacts(hook, view.approval, scope, team ? view : null)} />

      <SectionCard flat heading={t(TEXT_HEADINGS[hook.action.type])}>
        {#if hook.action.type === 'command'}
          <ScriptCode body={hook.action.command} language="sh" />
        {:else}
          <p class="chat-radius border border-line bg-canvas px-3.5 py-3 text-sm leading-relaxed break-words whitespace-pre-wrap text-fg"><HiddenText text={actionText(hook.action)} /></p>
        {/if}
      </SectionCard>
    {/if}

    <SectionCard flat heading={t('hooks.panel.actions')}>
      <HookActions {view} {lockReason} {sharing} {canUpdate} {copyLock} {busy} {onedit} {ondelete} {onshare} {onupdate} {onremove} {oncopy} />
    </SectionCard>
  {/if}
</SidePanel>
