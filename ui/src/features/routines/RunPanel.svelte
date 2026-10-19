<script lang="ts">
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import Square from '@lucide/svelte/icons/square';
  import SquareArrowOutUpRight from '@lucide/svelte/icons/square-arrow-out-up-right';

  import Button from '$lib/components/Button.svelte';
  import FactsCard from '$lib/components/FactsCard.svelte';
  import { type FactRow, momentFact } from '$lib/components/facts';
  import { ICON } from '$lib/components/icon';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import SectionCard from '$lib/components/SectionCard.svelte';
  import SidePanel from '$lib/components/SidePanel.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { RoutineRun } from '$lib/ipc/bindings/RoutineRun';
  import { cx } from '$lib/utils/cx';
  import { formatDateTime } from '$lib/utils/format';

  import HiddenText from '../memory/HiddenText.svelte';
  import { spelledOut } from '../memory/script-format';
  import { durationText, RUN_STATUS_LABELS, TRIGGER_LABELS } from './routine-format';
  import RunStatusBadge from './RunStatusBadge.svelte';

  interface Props {
    open: boolean;
    /** Fills the column beside the table instead of sliding in over it. */
    inline: boolean;
    run: RoutineRun | null;
    /** A change to this run is being written. */
    busy: boolean;
    onstop: () => void;
    onopenchat: () => void;
    onopensession: () => void;
    /** Opens the routine the run belongs to, in the list of routines. */
    onroutine: () => void;
    onclose: () => void;
  }

  let { open, inline, run, busy, onstop, onopenchat, onopensession, onroutine, onclose }: Props = $props();

  const warning = NOTICE_LOOKS.warning;

  const facts = $derived.by<FactRow[]>(() => {
    if (!run) {
      return [];
    }

    return [
      momentFact(t('routines.run.field.started'), run.startedAt),
      ...(run.endedAt ? [momentFact(t('routines.run.field.ended'), run.endedAt)] : []),
      ...(run.durationMs !== null ? [{ label: t('routines.run.field.duration'), value: durationText(run.durationMs) }] : []),
      { label: t('routines.run.field.trigger'), value: t(TRIGGER_LABELS[run.trigger]) },
      ...(run.exitCode !== null ? [{ label: t('routines.run.field.exitCode'), value: String(run.exitCode), mono: true, tone: run.exitCode === 0 ? undefined : ('danger' as const) }] : []),
      { label: t('routines.field.source'), value: t(run.origin === 'team' ? 'routines.source.team' : 'routines.run.source.local') },
      ...(run.project ? [{ label: t('routines.field.project'), value: spelledOut(run.project), mono: true, cutStart: true, title: spelledOut(run.project) }] : []),
    ];
  });
</script>

{#snippet output(text: string)}
  <pre class="chat-radius max-h-72 overflow-auto border border-line bg-canvas px-3.5 py-3 font-mono text-[13px] leading-5 break-words whitespace-pre-wrap text-fg"><code><HiddenText {text} /></code></pre>
{/snippet}

<SidePanel {open} {inline} title={run?.routineName ? spelledOut(run.routineName) : ''} subtitle={run ? `${t(RUN_STATUS_LABELS[run.status])} · ${formatDateTime(run.startedAt)}` : ''} {onclose}>
  {#if run}
    <SectionCard flat heading={t('routines.panel.status')}>
      <div class="flex flex-col gap-3">
        <div class="flex items-center gap-3"><RunStatusBadge {run} /></div>
        {#if run.message}<p class="text-sm break-words text-fg-2">{spelledOut(run.message)}</p>{/if}

        {#if run.needsYou}
          <div class={cx('flex flex-col gap-2 rounded-md p-3', warning.box)} role="status">
            <p class="text-sm font-medium text-fg">{t('routines.panel.waiting.title')}</p>
            <p class="text-sm text-fg-2">{t('routines.panel.waiting.body')}</p>
          </div>
        {/if}
      </div>
    </SectionCard>

    <FactsCard flat heading={t('routines.panel.details')} rows={facts} />

    {#if run.excerpt}
      <SectionCard flat heading={t(run.session ? 'routines.run.reply' : 'routines.run.output')}>
        {@render output(run.excerpt)}
      </SectionCard>
    {/if}

    {#if run.errorExcerpt}
      <SectionCard flat heading={t('routines.run.errors')}>
        {@render output(run.errorExcerpt)}
      </SectionCard>
    {/if}

    <SectionCard flat heading={t('routines.panel.actions')}>
      <div class="flex flex-wrap items-center gap-2">
        {#if run.status === 'running'}
          {#if run.needsYou}
            <Button variant="primary" onclick={onopenchat}>
              {#snippet leading()}<MessageSquare size={ICON.sm} aria-hidden="true" />{/snippet}
              {t('routines.panel.openChat')}
            </Button>
          {/if}
          <Button variant="dangerQuiet" disabled={busy} onclick={onstop}>
            {#snippet leading()}<Square size={ICON.sm} aria-hidden="true" />{/snippet}
            {t('routines.run.stop')}
          </Button>
        {/if}
        {#if run.session}
          <Button onclick={onopensession}>
            {#snippet leading()}<SquareArrowOutUpRight size={ICON.sm} aria-hidden="true" />{/snippet}
            {t('routines.panel.openSession')}
          </Button>
        {/if}
        <Button variant="ghost" onclick={onroutine}>{t('routines.run.showRoutine')}</Button>
      </div>
    </SectionCard>
  {/if}
</SidePanel>
