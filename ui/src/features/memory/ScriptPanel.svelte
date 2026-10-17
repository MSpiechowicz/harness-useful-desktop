<script lang="ts">
  import ShieldCheck from '@lucide/svelte/icons/shield-check';

  import Button from '$lib/components/Button.svelte';
  import FactsCard from '$lib/components/FactsCard.svelte';
  import { ICON } from '$lib/components/icon';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import SectionCard from '$lib/components/SectionCard.svelte';
  import SidePanel from '$lib/components/SidePanel.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { ScriptDetail } from '$lib/ipc/bindings/ScriptDetail';
  import type { ScriptSummary } from '$lib/ipc/bindings/ScriptSummary';
  import type { ViewStatus } from '$lib/stores/view-loader.svelte';
  import { cx } from '$lib/utils/cx';

  import BodyState from './BodyState.svelte';
  import PanelActions from './PanelActions.svelte';
  import { languageLabel, needsReview, scopeText } from './script-format';
  import { scriptFacts } from './script-facts';
  import ScriptCode from './ScriptCode.svelte';

  interface Props {
    open: boolean;
    /** In a column of its own beside the table, not sliding in over it. */
    inline: boolean;
    script: ScriptSummary | null;
    /** The script with its body; null until it was read. */
    detail: ScriptDetail | null;
    status: ViewStatus;
    error: string | null;
    onretry: () => void;
    /** Trusts the version on screen, so agents may run it. Absent until the code is loaded and is the version the list reports. */
    ontrust: (() => void) | undefined;
    trusting: boolean;
    ondelete: () => void;
    onclose: () => void;
  }

  let { open, inline, script, detail, status, error, onretry, ontrust, trusting, ondelete, onclose }: Props = $props();

  const look = NOTICE_LOOKS.warning;
  // The body read from the server is what an agent would run, so its hash wins over the list's once loaded.
  const facts = $derived(script ? scriptFacts(script, detail?.sha256 ?? script.sha256) : []);
</script>

<SidePanel {open} {inline} title={script?.name ?? ''} subtitle={script ? `${languageLabel(script.language)} · ${scopeText(script)}` : ''} mono {onclose}>
  {#if script}
    <SectionCard flat heading={t('memory.scripts.detail.description')}>
      <p class="text-sm leading-relaxed break-words text-fg-2">{script.description}</p>
    </SectionCard>

    <SectionCard flat heading={t('memory.scripts.detail.code')}>
      <BodyState loaded={detail !== null} {status} {error} failed={t('memory.scripts.body.failed')} loading={t('memory.scripts.body.loading')} {onretry}>
        <div class="flex flex-col gap-3">{#if detail}<ScriptCode body={detail.body} language={detail.language} />{/if}</div>
      </BodyState>
    </SectionCard>

    <!-- Trust pins the body that is on screen: it needs the code read and the same version the list reports. -->
    <PanelActions deleteLabel={t('memory.panel.delete')} {ondelete}>
      {#if needsReview(script)}
        <div class={cx('flex w-full flex-col items-start gap-3 rounded-md p-3', look.box)} role="status">
          <div class="flex min-w-0 items-start gap-3">
            <span class="flex h-5 shrink-0 items-center"><look.icon class={look.color} size={ICON.sm} aria-hidden="true" /></span>
            <div class="flex min-w-0 flex-col gap-0.5">
              <p class="text-sm font-medium text-fg">{t('memory.scripts.trust.noticeTitle')}</p>
              <p class="text-sm text-fg-2">{t('memory.scripts.trust.noticeBody')}</p>
            </div>
          </div>

          <Button variant="primary" loading={trusting} disabled={!ontrust} onclick={ontrust}>
            {#snippet leading()}<ShieldCheck size={ICON.sm} aria-hidden="true" />{/snippet}
            {t('memory.scripts.trust.action')}
          </Button>
        </div>
      {/if}
    </PanelActions>

    <FactsCard flat heading={t('memory.panel.details')} rows={facts} />
  {/if}
</SidePanel>
