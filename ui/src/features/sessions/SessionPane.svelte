<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';

  import Button from '$lib/components/Button.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import SplitPane from '$lib/components/SplitPane.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { forkInChat, resumeInChat } from '$lib/stores/session-actions';
  import { sessionDetail as detail } from '$lib/stores/sessions.svelte';

  import BranchTree from './BranchTree.svelte';
  import SessionHeader from './SessionHeader.svelte';
  import Transcript from './Transcript.svelte';

  interface Props {
    summary: SessionSummary;
    project: string;
  }

  let { summary, project }: Props = $props();

  let treeSize = $state(400);
  let pickedId = $state<string | null>(null);
  let focus = $state<{ id: string; nonce: number } | null>(null);

  /** A click in the tree picks the node (for forking) and brings its message into view. */
  function pick(id: string): void {
    pickedId = id;
    focus = { id, nonce: (focus?.nonce ?? 0) + 1 };
  }
  let busy = $state(false);

  const tree = $derived(detail.tree);
  const selectedId = $derived(pickedId ?? tree?.leafId ?? null);
  const selectedNode = $derived(tree?.nodes.find((node) => node.id === selectedId) ?? null);

  async function resume(): Promise<void> {
    busy = true;
    await resumeInChat(agentSessions, summary.ref, project);
    busy = false;
  }

  async function fork(): Promise<void> {
    if (!selectedNode) {
      return;
    }

    busy = true;
    await forkInChat(agentSessions, summary.ref, selectedNode.id, project);
    busy = false;
  }
</script>

<div class="flex size-full min-h-0 flex-col">
  <SessionHeader {summary} {project} touched={detail.touched} node={selectedNode} {busy} onresume={() => void resume()} onfork={() => void fork()} />

  <div class="min-h-0 flex-1">
    <SplitPane label={t('sessions.tree.resize')} bind:size={treeSize} min={260} minSecond={320}>
      {#snippet first()}
        <section aria-label={t('sessions.tree.section')} class="flex size-full min-h-0 flex-col">
          {#if detail.treeStatus === 'loading' || detail.treeStatus === 'idle'}
            <div class="flex flex-col gap-2 p-4" role="status" aria-label={t('sessions.tree.loading')}>
              <Skeleton height="1.25rem" />
              <Skeleton width="70%" height="1.25rem" />
              <Skeleton width="85%" height="1.25rem" />
            </div>
          {:else if detail.treeStatus === 'error' || !tree}
            <div class="flex flex-col items-center gap-3 p-6 text-center" role="alert">
              <CircleAlert size={24} class="text-danger" aria-hidden="true" />
              <p class="text-base text-fg-2">{detail.treeError?.message ?? t('sessions.tree.unreadable')}</p>
              <Button onclick={() => detail.reload()}>{t('sessions.tryAgain')}</Button>
            </div>
          {:else}
            <p class="tabular border-b border-line px-4 py-2 text-xs text-fg-3">
              {tn('sessions.tree.entries', tree.nodes.length)} · {tn('sessions.tree.branchPoints', tree.branchCount)}
              {#if tree.truncated}· {t('sessions.tree.truncated')}{/if}
            </p>
            <div class="min-h-0 flex-1">
              <BranchTree {tree} selected={selectedId} onselect={pick} />
            </div>
          {/if}
        </section>
      {/snippet}

      {#snippet second()}
        <Transcript {detail} {project} {focus} picked={selectedId} onpick={(id) => (pickedId = id)} />
      {/snippet}
    </SplitPane>
  </div>
</div>
