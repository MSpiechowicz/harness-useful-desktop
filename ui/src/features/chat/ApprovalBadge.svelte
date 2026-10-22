<script lang="ts">
  import ShieldAlert from '@lucide/svelte/icons/shield-alert';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import ShieldCheck from '@lucide/svelte/icons/shield-check';
  import ShieldHalf from '@lucide/svelte/icons/shield-half';
  import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import type { ApprovalMode } from '$lib/ipc/bindings/ApprovalMode';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import { activeMode, MODE_ORDER, MODES, nativeModeLabel } from './approval-modes';
  import ApprovalOption from './ApprovalOption.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  const CLOSE_MS = 220;
  const MODE_ICONS = { alwaysAsk: ShieldCheck, write: ShieldHalf, yolo: ShieldAlert } satisfies Record<ApprovalMode, unknown>;

  /** `null` is "use the harness configuration". */
  let choice = $state<ApprovalMode | null>(null);
  let restarting = $state(false);

  const mode = $derived(activeMode(session.info.approvalOverride, session.info.configuredApprovalMode));
  const info = $derived(mode ? MODES[mode] : null);
  const source = $derived(session.info.approvalOverride ? t('chat.approval.source.session') : t('chat.approval.source.config'));
  // A harness with approval modes that reports none of its own (OpenCode) follows its settings until a mode is chosen here.
  const harnessName = $derived(harness.get(session.info.harness)?.info.name ?? '');
  // Yolo is offered only where the harness can run without asking (Claude Code cannot).
  const options = $derived(MODE_ORDER.filter((option) => option !== 'yolo' || harness.get(session.info.harness)?.capabilities.yoloMode));
  // A harness with a mode of its own (Claude Code) shows it, read-only, while the app has no mode of its own for the chat.
  const native = $derived(info ? null : session.info.nativeApprovalMode);
  const nativeName = $derived(native ? t('chat.approval.native', { harness: harnessName, mode: nativeModeLabel(native) }) : null);
  const modeName = $derived(info?.label ?? nativeName ?? t('chat.approval.settings', { harness: harnessName }));
  const tone = $derived(info?.tone ?? (native?.unattended ? 'warning' : 'neutral'));
  const configured = $derived(session.info.configuredApprovalMode);
  const changed = $derived(choice !== session.info.approvalOverride);

  $effect(() => {
    if (session.ui.approvalChooser) {
      choice = session.info.approvalOverride;
    }
  });

  async function apply(): Promise<void> {
    restarting = true;

    // Let the popover finish closing before its session (and this badge) is replaced.
    session.ui.approvalChooser = false;
    await new Promise((resolve) => setTimeout(resolve, CLOSE_MS));

    await agentSessions.restart(session.key, { approvalOverride: choice });
  }
</script>

<Popover label={t('chat.approval.popover')} kind="dialog" bind:open={session.ui.approvalChooser} class="w-[22rem]">
  {#snippet trigger({ props })}
    <Tooltip text={info ? t('chat.approval.tooltip', { explanation: info.explanation, source }) : t('chat.approval.settingsTooltip', { harness: harnessName })} side="bottom">
      {#snippet children({ describedby })}
        <button type="button" {...props} aria-describedby={describedby} class="rounded-md" aria-label={t('chat.approval.aria', { mode: modeName })}>
          <Badge {tone} class="h-control rounded-md! px-2.5 text-sm!">
            {#if tone === 'warning'}<ShieldAlert size={16} aria-hidden="true" />{:else}<ShieldCheck size={16} aria-hidden="true" />{/if}
            {nativeName ?? t('chat.approval.badge', { mode: modeName })}
          </Badge>
        </button>
      {/snippet}
    </Tooltip>
  {/snippet}

  {#snippet children()}
    <div class="flex flex-col">
      <header class="flex flex-col gap-0.5 px-2.5 pt-2 pb-3">
        <h2 class="text-sm font-semibold text-fg">{t('chat.approval.heading')}</h2>
        <p class="text-xs text-fg-3">{t('chat.approval.subtitle')}</p>
      </header>

      <div class="flex flex-col gap-0.5" role="radiogroup" aria-label={t('chat.approval.group')}>
        <ApprovalOption
          label={t('chat.approval.useConfig')}
          description={configured ? t('chat.approval.useConfig.known', { mode: MODES[configured].label }) : t('chat.approval.useConfig.unknown', { harness: harnessName })}
          icon={SlidersHorizontal}
          tone="neutral"
          selected={choice === null}
          current={session.info.approvalOverride === null}
          onselect={() => (choice = null)}
        />
        {#each options as option (option)}
          <ApprovalOption
            label={MODES[option].label}
            description={MODES[option].explanation}
            icon={MODE_ICONS[option]}
            tone={MODES[option].tone}
            selected={choice === option}
            current={session.info.approvalOverride === option}
            onselect={() => (choice = option)}
          />
        {/each}
      </div>

      <footer class="mt-1.5 flex flex-col gap-4 border-t border-line px-2.5 pt-3 pb-1.5">
        <p class="flex items-start gap-1.5 text-xs text-fg-3">
          <RotateCcw size={14} aria-hidden="true" class="mt-px shrink-0" />
          {t('chat.approval.restartNote')}
        </p>
        <Button variant="primary" disabled={!changed} loading={restarting} onclick={apply}>{t('chat.approval.restart')}</Button>
      </footer>
    </div>
  {/snippet}
</Popover>
