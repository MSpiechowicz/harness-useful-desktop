<script lang="ts">
  import GitFork from '@lucide/svelte/icons/git-fork';
  import Play from '@lucide/svelte/icons/play';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { SessionNode } from '$lib/ipc/bindings/SessionNode';
  import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
  import { formatBytes, relativeTime } from '$lib/utils/format';

  import { sessionTitle } from './session-format';
  import TouchedFiles from './TouchedFiles.svelte';

  interface Props {
    summary: SessionSummary;
    project: string;
    touched: string[];
    /** The tree node picked for forking. */
    node: SessionNode | null;
    busy: boolean;
    onresume: () => void;
    onfork: () => void;
  }

  let { summary, project, touched, node, busy, onresume, onfork }: Props = $props();

  const canFork = $derived(node?.kind === 'user');
</script>

<header class="flex flex-col gap-2.5 border-b border-line px-5 py-3">
  <!-- The actions sit on the title line, their top edge level with the top of the title's capitals (7px into its line). -->
  <div class="flex flex-wrap items-start gap-3">
    <div class="flex min-w-0 flex-1 flex-col">
      <h2 class="truncate text-xl text-fg" title={summary.title ?? summary.firstPrompt ?? undefined}>{sessionTitle(summary)}</h2>
      <!-- Always as tall as the "Forked" badge, so the header does not jump when a session has one. -->
      <p class="tabular flex h-5 items-center gap-x-2 overflow-hidden text-xs whitespace-nowrap text-fg-3">
        <span>{t('sessions.header.updated', { time: relativeTime(summary.updatedAt) })}</span>
        {#if summary.model}<span>· {summary.model}</span>{/if}
        <span>· {formatBytes(summary.sizeBytes)}</span>
        {#if summary.parentSession}<Badge tone="info">{t('sessions.header.forked')}</Badge>{/if}
      </p>
    </div>

    <div class="mt-[7px] flex items-center gap-2">
      <Button
        variant="secondary"
        size="sm"
        disabled={!canFork || busy}
        title={canFork ? t('sessions.header.forkHint') : t('sessions.header.forkNeedsPick')}
        onclick={onfork}
      >
        {#snippet leading()}<GitFork size={16} aria-hidden="true" />{/snippet}
        {t('sessions.header.fork')}
      </Button>
      <Button variant="primary" size="sm" loading={busy} onclick={onresume}>
        {#snippet leading()}<Play size={16} aria-hidden="true" />{/snippet}
        {t('sessions.header.resume')}
      </Button>
    </div>
  </div>

  <TouchedFiles {project} paths={touched} />
</header>
