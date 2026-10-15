<script lang="ts">
  import ShieldAlert from '@lucide/svelte/icons/shield-alert';

  import { MODE_ORDER, MODES } from '../chat/approval-modes';
  import Badge from '$lib/components/Badge.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  import { WIDTH } from './layout';
  import StepHeader from './StepHeader.svelte';

  /** How each OMP mode behaves in practice, next to its name. */
  const DETAILS = $derived<Record<(typeof MODE_ORDER)[number], { cli: string; when: string; good: string }>>({
    alwaysAsk: { cli: 'always-ask', when: t('onboarding.approval.alwaysAsk.when'), good: t('onboarding.approval.alwaysAsk.good') },
    write: { cli: 'write', when: t('onboarding.approval.write.when'), good: t('onboarding.approval.write.good') },
    yolo: { cli: 'yolo', when: t('onboarding.approval.yolo.when'), good: t('onboarding.approval.yolo.good') },
  });

  const TONE: Record<string, string> = {
    success: 'border-success/30',
    info: 'border-accent/30',
    warning: 'border-warning/40 bg-warning-wash',
  };
</script>

<div class={cx('mx-auto flex w-full flex-col gap-8 [@media(max-height:720px)]:gap-4', WIDTH.wide)}>
  <StepHeader
    eyebrow={t('onboarding.approval.eyebrow')}
    title={t('onboarding.approval.title')}
    body={t('onboarding.approval.body')}
  />

  <ul class="grid grid-cols-3 gap-4" aria-label={t('onboarding.approval.modes')}>
    {#each MODE_ORDER as mode (mode)}
      {@const info = MODES[mode]}
      <li class={cx('flex flex-col gap-2 rounded-lg border bg-glass p-5', TONE[info.tone])}>
        <div class="flex items-center justify-between gap-2">
          <p class="text-base font-medium text-fg first-letter:uppercase">{info.label}</p>
          {#if mode === 'yolo'}<Badge tone="warning" size="lg">{t('onboarding.approval.default')}</Badge>{/if}
        </div>
        <code class="font-mono text-xs text-fg-3">--approval-mode {DETAILS[mode].cli}</code>
        <p class="text-sm text-fg-2">{DETAILS[mode].when}</p>
        <p class="mt-auto border-t border-line pt-2 text-xs text-fg-3">{DETAILS[mode].good}</p>
      </li>
    {/each}
  </ul>

  <div class="flex items-start gap-4 rounded-lg border border-line bg-glass p-5">
    <ShieldAlert class="mt-0.5 shrink-0 text-warning" size={20} aria-hidden="true" />
    <ul class="flex list-disc flex-col gap-2 pl-5 text-sm text-fg-2 marker:text-warning">
      <li>{t('onboarding.approval.respects.before')}<code class="font-mono text-xs">tools.approvalMode</code>{t('onboarding.approval.respects.after')}</li>
      <li>{t('onboarding.approval.visible')}</li>
      <li>{t('onboarding.approval.stricter')}</li>
    </ul>
  </div>
</div>
