<script lang="ts">
  import ShieldAlert from '@lucide/svelte/icons/shield-alert';

  import { MODE_ORDER, MODES, nativeModeLabel } from '../chat/approval-modes';
  import Badge from '$lib/components/Badge.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { cx } from '$lib/utils/cx';

  import type { ApprovalNotice } from './approval-notices';
  import { WIDTH } from './layout';
  import StepHeader from './StepHeader.svelte';

  interface Props {
    /** Whose notice this is: OMP and OpenCode run tools without asking by default, Claude Code only when its settings say so. Each is configured in its own place. */
    notice: ApprovalNotice;
  }

  let { notice }: Props = $props();

  interface ModeCard {
    id: string;
    label: string;
    tone: 'success' | 'info' | 'warning';
    /** The flag or setting that selects it, as the user would type it. */
    setting: string;
    when: string;
    good: string;
    /** Marks the harness's own default. */
    isDefault: boolean;
  }

  /** How each OMP mode behaves in practice, next to its name. */
  const OMP_DETAILS = $derived<Record<(typeof MODE_ORDER)[number], { cli: string; when: string; good: string }>>({
    alwaysAsk: { cli: 'always-ask', when: t('onboarding.approval.alwaysAsk.when'), good: t('onboarding.approval.alwaysAsk.good') },
    write: { cli: 'write', when: t('onboarding.approval.write.when'), good: t('onboarding.approval.write.good') },
    yolo: { cli: 'yolo', when: t('onboarding.approval.yolo.when'), good: t('onboarding.approval.yolo.good') },
  });

  const OMP_CARDS = $derived<ModeCard[]>(
    MODE_ORDER.map((mode) => ({
      id: mode,
      label: MODES[mode].label,
      tone: MODES[mode].tone,
      setting: `--approval-mode ${OMP_DETAILS[mode].cli}`,
      when: OMP_DETAILS[mode].when,
      good: OMP_DETAILS[mode].good,
      isDefault: mode === 'yolo',
    })),
  );

  /** OpenCode decides per tool: each `permission` entry is allow, ask or deny. */
  const OPENCODE_CARDS = $derived<ModeCard[]>([
    { id: 'deny', label: t('onboarding.approval.oc.deny.label'), tone: 'info', setting: '"bash": "deny"', when: t('onboarding.approval.oc.deny.when'), good: t('onboarding.approval.oc.deny.good'), isDefault: false },
    { id: 'ask', label: t('onboarding.approval.oc.ask.label'), tone: 'success', setting: '"bash": "ask"', when: t('onboarding.approval.oc.ask.when'), good: t('onboarding.approval.oc.ask.good'), isDefault: false },
    { id: 'allow', label: t('onboarding.approval.oc.allow.label'), tone: 'warning', setting: '"bash": "allow"', when: t('onboarding.approval.oc.allow.when'), good: t('onboarding.approval.oc.allow.good'), isDefault: true },
  ]);

  /** Claude Code names its modes in `permissions.defaultMode`; the three that matter here run from asking to nothing waiting. */
  const CLAUDE_CARDS = $derived<ModeCard[]>([
    { id: 'default', label: nativeModeLabel({ name: 'default', unattended: false }), tone: 'success', setting: '"defaultMode": "default"', when: t('onboarding.approval.cc.default.when'), good: t('onboarding.approval.cc.default.good'), isDefault: false },
    { id: 'acceptEdits', label: nativeModeLabel({ name: 'acceptEdits', unattended: false }), tone: 'info', setting: '"defaultMode": "acceptEdits"', when: t('onboarding.approval.cc.edits.when'), good: t('onboarding.approval.cc.edits.good'), isDefault: false },
    { id: 'bypassPermissions', label: nativeModeLabel({ name: 'bypassPermissions', unattended: true }), tone: 'warning', setting: '"defaultMode": "bypassPermissions"', when: t('onboarding.approval.cc.bypass.when'), good: t('onboarding.approval.cc.bypass.good'), isDefault: false },
  ]);

  const CARDS = $derived({ omp: OMP_CARDS, opencode: OPENCODE_CARDS, claude: CLAUDE_CARDS });
  const cards = $derived(CARDS[notice]);
  const claudeMode = $derived.by(() => {
    const native = harness.get('claude')?.nativeApprovalMode;

    return native ? nativeModeLabel(native) : '';
  });

  const MODES_LABEL = $derived({ omp: t('onboarding.approval.modes'), opencode: t('onboarding.approval.oc.modes'), claude: t('onboarding.approval.cc.modes') });

  const TONE: Record<ModeCard['tone'], string> = {
    success: 'border-success/30',
    info: 'border-accent/30',
    warning: 'border-warning/40 bg-warning-wash',
  };
</script>

<div class={cx('mx-auto flex w-full flex-col gap-8 [@media(max-height:720px)]:gap-4', WIDTH.wide)}>
  {#if notice === 'opencode'}
    <StepHeader eyebrow={t('onboarding.approval.eyebrow')} title={t('onboarding.approval.oc.title')} body={t('onboarding.approval.oc.body')} />
  {:else if notice === 'claude'}
    <StepHeader eyebrow={t('onboarding.approval.eyebrow')} title={t('onboarding.approval.cc.title')} body={t('onboarding.approval.cc.body', { mode: claudeMode })} />
  {:else}
    <StepHeader eyebrow={t('onboarding.approval.eyebrow')} title={t('onboarding.approval.title')} body={t('onboarding.approval.body')} />
  {/if}

  <ul class="grid grid-cols-3 gap-4" aria-label={MODES_LABEL[notice]}>
    {#each cards as card (card.id)}
      <li class={cx('flex flex-col gap-2 rounded-lg border bg-glass p-5', TONE[card.tone])}>
        <div class="flex items-center justify-between gap-2">
          <p class="text-base font-medium text-fg first-letter:uppercase">{card.label}</p>
          {#if card.isDefault}<Badge tone="warning" size="lg">{notice === 'opencode' ? t('onboarding.approval.oc.default') : t('onboarding.approval.default')}</Badge>{/if}
        </div>
        <code class="font-mono text-xs text-fg-3">{card.setting}</code>
        <p class="text-sm text-fg-2">{card.when}</p>
        <p class="mt-auto border-t border-line pt-2 text-xs text-fg-3">{card.good}</p>
      </li>
    {/each}
  </ul>

  <div class="flex items-start gap-4 rounded-lg border border-line bg-glass p-5">
    <ShieldAlert class="mt-0.5 shrink-0 text-warning" size={20} aria-hidden="true" />
    <ul class="flex list-disc flex-col gap-2 pl-5 text-sm text-fg-2 marker:text-warning">
      {#if notice === 'claude'}
        <li>{t('onboarding.approval.cc.respects.before')}<code class="font-mono text-xs">permissions.defaultMode</code>{t('onboarding.approval.cc.respects.after')}</li>
        <li>{t('onboarding.approval.visible')}</li>
        <li>{t('onboarding.approval.cc.stricter')}</li>
      {:else if notice === 'opencode'}
        <li>{t('onboarding.approval.oc.respects.before')}<code class="font-mono text-xs">permission</code>{t('onboarding.approval.oc.respects.after')}</li>
        <li>{t('onboarding.approval.oc.guarded')}</li>
        <li>{t('onboarding.approval.oc.stricter')}</li>
      {:else}
        <li>{t('onboarding.approval.respects.before')}<code class="font-mono text-xs">tools.approvalMode</code>{t('onboarding.approval.respects.after')}</li>
        <li>{t('onboarding.approval.visible')}</li>
        <li>{t('onboarding.approval.stricter')}</li>
      {/if}
    </ul>
  </div>
</div>
