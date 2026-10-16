<script lang="ts">
  import { fade, fly } from 'svelte/transition';

  import { focusTrap } from '$lib/a11y/focus-trap';
  import { scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { forkInChat } from '$lib/stores/session-actions';
  import { sessionDetail as detail } from '$lib/stores/sessions.svelte';

  import SessionDetails from './SessionDetails.svelte';
  import SessionHeader from './SessionHeader.svelte';
  import Transcript from './Transcript.svelte';

  interface Props {
    summary: SessionSummary;
    project: string;
    /** How many sessions were forked from this one. */
    forks: number;
    /** The session is being resumed in a chat; forking waits for it. */
    resuming?: boolean;
  }

  let { summary, project, forks, resuming = false }: Props = $props();

  /** Below this width the details column moves into a sheet so the transcript keeps its room. */
  const SHEET_BELOW_PX = 900;

  let width = $state(0);
  let sheetOpen = $state(false);
  let busy = $state(false);

  // Forking starts a chat, and a branching tree exists only in harnesses that have one.
  const tree = $derived(harness.get(summary.ref.harness));
  const forkable = $derived(harness.canChat(summary.ref.harness));

  const folded = $derived(width > 0 && width < SHEET_BELOW_PX);

  // Widening the pane past the breakpoint turns the sheet back into a column.
  $effect(() => {
    if (!folded) {
      sheetOpen = false;
    }
  });

  function closeSheet(): void {
    sheetOpen = false;
    document.querySelector<HTMLElement>('[data-details-toggle]')?.focus();
  }

  async function fork(entryId: string): Promise<void> {
    busy = true;
    await forkInChat(agentSessions, summary.ref, entryId, project);
    busy = false;
  }
</script>

<div bind:clientWidth={width} class="relative flex size-full min-h-0 flex-col">
  <SessionHeader {summary} detailsFolded={folded} ondetails={() => (sheetOpen = !sheetOpen)} />

  <div class="flex min-h-0 flex-1">
    <div class="min-w-0 flex-1">
      <Transcript {detail} {project} busy={busy || resuming} onfork={forkable ? (id) => void fork(id) : undefined} />
    </div>

    {#if !folded}
      <SessionDetails {summary} stats={detail.stats} touched={detail.touched} {project} {forks} showForks={tree?.capabilities.sessionTree ?? true} class="w-[300px] shrink-0 border-l border-line" />
    {/if}
  </div>

  {#if folded && sheetOpen}
    <button type="button" tabindex="-1" aria-hidden="true" class="absolute inset-0 z-30 cursor-default bg-scrim backdrop-blur-sm" transition:fade={{ duration: scaled(160) }} onclick={closeSheet}></button>

    <div
      role="dialog"
      aria-label={t('sessions.details.label')}
      use:focusTrap={{ onEscape: closeSheet }}
      transition:fly={{ x: 32, duration: scaled(240) }}
      class="absolute inset-y-0 right-0 z-40 flex w-[min(21rem,92%)] flex-col border-l border-line-strong bg-surface-1 shadow-pop outline-none"
    >
      <SessionDetails {summary} stats={detail.stats} touched={detail.touched} {project} {forks} showForks={tree?.capabilities.sessionTree ?? true} class="min-h-0 flex-1" />
    </div>
  {/if}
</div>
