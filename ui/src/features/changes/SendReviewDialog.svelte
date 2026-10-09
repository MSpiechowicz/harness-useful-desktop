<script lang="ts">
  import { onMount, untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import Select from '$lib/components/Select.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import type { StripOption } from '$lib/components/strip';
  import type { ComposedReview } from '$lib/ipc/bindings/ComposedReview';
  import { IpcFailure } from '$lib/ipc/transport';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { review } from '$lib/stores/review.svelte';

  interface Props {
    project: string;
    onclose: () => void;
  }

  let { project, onclose }: Props = $props();

  const NEW_SESSION = 'new';

  let open = $state(true);
  let composed = $state.raw<ComposedReview | null>(null);
  let failure = $state<string | null>(null);
  let sending = $state(false);

  const chats = $derived(agentSessions.sessions.filter((session) => session.info.project === project && !session.ended));
  const canStartNew = $derived(harness.active?.detection.installed ?? false);

  const options = $derived<StripOption<string>[]>([
    ...chats.map((session) => ({ value: session.key, label: `${session.name ?? 'New chat'} · ${session.streaming ? 'busy, sends as follow-up' : 'idle'}` })),
    ...(canStartNew ? [{ value: NEW_SESSION, label: 'New session' }] : []),
  ]);

  // Prefer the chat the user is looking at, else the first open one, else a new session.
  function defaultTarget(): string {
    const open = agentSessions.sessions.filter((session) => session.info.project === project && !session.ended);
    const showing = open.find((session) => session.key === agentSessions.activeKey);

    return (showing ?? open[0])?.key ?? NEW_SESSION;
  }

  let target = $state<string>(untrack(defaultTarget));

  onMount(async () => {
    try {
      composed = await review.compose(project);
    } catch (error) {
      failure = IpcFailure.from(error).message;
    }
  });

  async function send(): Promise<void> {
    const prompt = composed?.prompt;
    const harnessId = harness.active?.info.id;
    if (prompt === undefined || harnessId === undefined) {
      return;
    }

    sending = true;
    const delivered = await review.send(project, harnessId, target, prompt);
    sending = false;

    if (delivered) {
      onclose();
    }
  }
</script>

<Dialog bind:open title="Review prompt" description="This exact text is sent to the agent." size="lg" {onclose}>
  {#if failure}
    <p class="text-base text-danger" role="alert">{failure}</p>
  {:else if composed}
    <p class="mb-2 text-xs text-fg-3 tabular">{composed.commentCount} {composed.commentCount === 1 ? 'comment' : 'comments'} · {composed.fileCount} {composed.fileCount === 1 ? 'file' : 'files'} · {composed.bytes} bytes</p>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
    <pre tabindex="0" aria-label="Prompt to send" class="max-h-[40vh] overflow-auto rounded-md border border-line bg-surface-2 p-3 font-mono text-xs leading-relaxed break-words whitespace-pre-wrap text-fg">{composed.prompt}</pre>

    <div class="mt-4 flex items-center gap-3">
      <span class="text-sm text-fg-2">Send to</span>
      <Select {options} bind:value={target} label="Send to" class="min-w-0 flex-1" />
    </div>
  {:else}
    <div class="flex flex-col gap-2" role="status" aria-label="Composing the prompt">
      <Skeleton height="1rem" />
      <Skeleton height="1rem" />
      <Skeleton width="60%" height="1rem" />
    </div>
  {/if}

  {#snippet footer()}
    <Button variant="ghost" onclick={onclose}>Cancel</Button>
    <Button variant="primary" loading={sending} disabled={!composed || options.length === 0} onclick={() => void send()}>Send to agent</Button>
  {/snippet}
</Dialog>
