<script lang="ts">
  import GitFork from '@lucide/svelte/icons/git-fork';
  import Play from '@lucide/svelte/icons/play';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
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
  <div class="flex flex-wrap items-center gap-3">
    <div class="flex min-w-0 flex-1 flex-col">
      <h2 class="truncate text-xl text-fg" title={summary.title ?? summary.firstPrompt ?? undefined}>{sessionTitle(summary)}</h2>
      <p class="tabular flex flex-wrap items-center gap-x-2 text-xs text-fg-3">
        <span>Updated {relativeTime(summary.updatedAt)}</span>
        {#if summary.model}<span>· {summary.model}</span>{/if}
        <span>· {formatBytes(summary.sizeBytes)}</span>
        {#if summary.parentSession}<Badge tone="info">Forked</Badge>{/if}
      </p>
    </div>

    <div class="flex items-center gap-2">
      <Button
        variant="secondary"
        disabled={!canFork || busy}
        title={canFork ? 'Start a new session from the selected message' : 'Select one of your messages in the tree to fork from it'}
        onclick={onfork}
      >
        {#snippet leading()}<GitFork size={16} aria-hidden="true" />{/snippet}
        Fork from here
      </Button>
      <Button variant="primary" loading={busy} onclick={onresume}>
        {#snippet leading()}<Play size={16} aria-hidden="true" />{/snippet}
        Resume in chat
      </Button>
    </div>
  </div>

  <TouchedFiles {project} paths={touched} />
</header>
