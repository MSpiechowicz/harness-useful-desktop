<script lang="ts">
  import Pause from '@lucide/svelte/icons/pause';
  import Pencil from '@lucide/svelte/icons/pencil';
  import Trash2 from '@lucide/svelte/icons/trash-2';

  import Button from '$lib/components/Button.svelte';
  import FactsCard from '$lib/components/FactsCard.svelte';
  import { ICON } from '$lib/components/icon';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import SectionCard from '$lib/components/SectionCard.svelte';
  import SidePanel from '$lib/components/SidePanel.svelte';
  import Switch from '$lib/components/Switch.svelte';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MessageKey } from '$lib/i18n/locales/en';
  import type { HookView } from '$lib/ipc/bindings/HookView';
    import { nav } from '$lib/stores/nav.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';
  import { cx } from '$lib/utils/cx';

  import HiddenText from '../memory/HiddenText.svelte';
  import ScriptCode from '../memory/ScriptCode.svelte';
  import SettingRow from '../settings/SettingRow.svelte';
  import FamilyInstall from './FamilyInstall.svelte';
  import { hookFacts } from './hook-facts';
  import { actionText, EVENT_LABELS, EVENT_WHEN, hookTitle, MEMBER_LABELS, SCOPE_LABELS, waitingFor } from './hook-format';
  import HookReview from './HookReview.svelte';

  interface Props {
    open: boolean;
    /** In a column of its own beside the list, not sliding in over it. */
    inline: boolean;
    view: HookView | null;
    /** A change to this hook is being written. */
    busy: boolean;
    ontoggle: (enabled: boolean) => void;
    onapprove: () => void;
    onreject: () => void;
    onedit: () => void;
    ondelete: () => void;
    onclose: () => void;
  }

  let { open, inline, view, busy, ontoggle, onapprove, onreject, onedit, ondelete, onclose }: Props = $props();

  const hook = $derived(view?.hook ?? null);
  const info = NOTICE_LOOKS.info;
  const danger = NOTICE_LOOKS.error;

  const STATUS_TEXT: Record<string, MessageKey> = {
    active: 'hooks.panel.status.active',
    off: 'hooks.panel.status.off',
    pending: 'hooks.panel.status.pending',
    paused: 'hooks.panel.status.paused',
    invalid: 'hooks.panel.status.invalid',
  };

  const TEXT_HEADINGS = { addContext: 'hooks.field.context', followUp: 'hooks.field.message', command: 'hooks.field.command' } as const;

  const waiting = $derived(hook && view?.status === 'paused' ? waitingFor(hook, prefs.value?.memoryEnabled !== false) : null);
  // Why the switch and Edit are off: both would write the file, and a write approves what it writes.
  const lockReason = $derived(view?.status === 'pending' ? t('hooks.locked.pending') : view?.hook === null ? t('hooks.locked.invalid') : null);

  function openSettings(): void {
    viewState.settingsTarget = 'memory';
    nav.go('settings');
  }
</script>

<SidePanel
  {open}
  {inline}
  title={view ? hookTitle(view) : ''}
  subtitle={hook ? `${t(EVENT_LABELS[hook.event])} · ${t(SCOPE_LABELS[hook.scope])}` : (view?.id ?? '')}
  {onclose}
>
  {#if view}
    <SectionCard flat heading={t('hooks.panel.status')}>
      {#if hook}
        <div class="-my-4">
          <SettingRow title={t('hooks.panel.enabled')} description={t(STATUS_TEXT[view.status], { when: t(EVENT_WHEN[hook.event]) })}>
            {#snippet control()}
              <Tooltip text={lockReason ?? ''} side="bottom" disabled={lockReason === null}>
                {#snippet children({ describedby })}
                  <span aria-describedby={describedby} class="inline-flex">
                    <Switch
                      label={t('hooks.panel.enabledLabel', { name: hookTitle(view) })}
                      disabled={lockReason !== null || busy}
                      bind:checked={() => view.status !== 'pending' && hook.enabled, (next) => ontoggle(next)}
                    />
                  </span>
                {/snippet}
              </Tooltip>
            {/snippet}
          </SettingRow>
        </div>
      {/if}

      {#if view.status === 'invalid'}
        <div class={cx('flex items-start gap-3 rounded-md p-3', danger.box)} role="alert">
          <span class="flex h-5 shrink-0 items-center"><danger.icon class={danger.color} size={ICON.sm} aria-hidden="true" /></span>
          <div class="flex min-w-0 flex-col gap-0.5">
            <p class="text-sm font-medium text-fg">{t('hooks.panel.invalid.title')}</p>
            <p class="text-sm break-words text-fg-2">{view.problem ?? t('hooks.panel.invalid.unknown')}</p>
            <p class="mt-1 font-mono text-xs break-all text-fg-3">{view.id}.json</p>
          </div>
        </div>
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
        <HookReview {view} {busy} {onapprove} {onreject} />
      </SectionCard>
    {/if}

    {#if hook}
      {#if hook.description}
        <SectionCard flat heading={t('hooks.field.description')}>
          <p class="text-sm leading-relaxed break-words text-fg-2"><HiddenText text={hook.description} /></p>
        </SectionCard>
      {/if}

      <FactsCard flat heading={t('hooks.panel.details')} rows={hookFacts(hook, view.approval)} />

      <SectionCard flat heading={t(TEXT_HEADINGS[hook.action.type])}>
        {#if hook.action.type === 'command'}
          <ScriptCode body={hook.action.command} language="sh" />
        {:else}
          <p class="chat-radius border border-line bg-canvas px-3.5 py-3 text-sm leading-relaxed break-words whitespace-pre-wrap text-fg"><HiddenText text={actionText(hook.action)} /></p>
        {/if}
      </SectionCard>
    {/if}

    <SectionCard flat heading={t('hooks.panel.actions')}>
      <div class="flex flex-wrap items-center gap-2">
        <!-- A save approves the bytes it writes, so a hook that waits for approval cannot be edited: the user approves or rejects it first. -->
        <Tooltip text={lockReason ?? ''} disabled={lockReason === null}>
          {#snippet children({ describedby })}
            <Button disabled={lockReason !== null} aria-describedby={describedby} onclick={onedit}>
              {#snippet leading()}<Pencil size={ICON.sm} aria-hidden="true" />{/snippet}
              {t('hooks.panel.edit')}
            </Button>
          {/snippet}
        </Tooltip>
        <Button variant="dangerQuiet" onclick={ondelete}>
          {#snippet leading()}<Trash2 size={ICON.sm} aria-hidden="true" />{/snippet}
          {t('hooks.panel.delete')}
        </Button>
      </div>
    </SectionCard>
  {/if}
</SidePanel>
