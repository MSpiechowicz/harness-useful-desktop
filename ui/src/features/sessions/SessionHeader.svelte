<script lang="ts">
  import PanelRightOpen from '@lucide/svelte/icons/panel-right-open';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
  import { harness } from '$lib/stores/harness.svelte';
  import { relativeTime } from '$lib/utils/format';

  import { sessionTitle } from './session-format';

  interface Props {
    summary: SessionSummary;
    /** True while the details column is folded away; the header then offers a button that opens it as a sheet. */
    detailsFolded: boolean;
    ondetails: () => void;
  }

  let { summary, detailsFolded, ondetails }: Props = $props();
</script>

<header class="flex items-center gap-4 border-b border-line px-6 py-3">
  <div class="flex min-w-0 flex-1 flex-col">
    <h2 class="truncate text-xl text-fg" title={summary.title ?? summary.firstPrompt ?? undefined}>{sessionTitle(summary)}</h2>
    <!-- Only what the details beside the transcript do not say: which harness, and when. Always as tall as the badge,
         so the header does not jump when a session has one. -->
    <p class="tabular flex h-5 items-center gap-x-2 overflow-hidden text-xs whitespace-nowrap text-fg-3">
      <span>{harness.get(summary.ref.harness)?.info.name ?? summary.ref.harness}</span>
      <span>· {t('sessions.header.updated', { time: relativeTime(summary.updatedAt) })}</span>
      {#if summary.emptyCopy}
        <Badge>{t('sessions.header.emptyCopy')}</Badge>
      {:else if summary.parentSession}
        <Badge tone="info">{t('sessions.header.forked')}</Badge>
      {/if}
    </p>
  </div>

  {#if detailsFolded}
    <Button data-details-toggle variant="ghost" aria-haspopup="dialog" onclick={ondetails}>
      {#snippet leading()}<PanelRightOpen size={ICON.sm} aria-hidden="true" />{/snippet}
      {t('sessions.header.details')}
    </Button>
  {/if}
</header>
