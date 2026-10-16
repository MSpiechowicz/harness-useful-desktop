<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import MessageSquare from '@lucide/svelte/icons/message-square';
  import Plus from '@lucide/svelte/icons/plus';
  import { onMount, untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import Markdown from '$lib/components/Markdown.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';
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
  const canStartNew = $derived(harness.chatActive?.detection.installed ?? false);

  /** Where the review can go: each open chat of the project, then a new session. */
  const options = $derived<{ value: string; title: string; detail: string; fresh: boolean }[]>([
    ...chats.map((session) => ({
      value: session.key,
      title: agentSessions.title(session.key),
      detail: session.streaming ? t('changes.send.busy') : t('changes.send.idle'),
      fresh: false,
    })),
    ...(canStartNew ? [{ value: NEW_SESSION, title: t('changes.send.new'), detail: t('changes.send.newDetail'), fresh: true }] : []),
  ]);

  /**
   * Prefer the chat that made the commented changes (its tools edited those files), then the chat the user is
   * looking at, then the first open one, else a new session.
   */
  function defaultTarget(): string {
    const open = agentSessions.sessions.filter((session) => session.info.project === project && !session.ended);
    const paths = new Set(review.commentsFor(project).map((comment) => comment.path));
    const author = [...open].reverse().find((session) => Object.values(session.chat.toolCalls).some((call) => call.touchedPaths.some((path) => paths.has(path))));
    const showing = open.find((session) => session.key === agentSessions.activeKey);

    return (author ?? showing ?? open[0])?.key ?? NEW_SESSION;
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
    const harnessId = harness.chatActive?.info.id;
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

<Dialog bind:open title={t('changes.send.title')} description={t('changes.send.description')} size="lg" {onclose}>
  {#if failure}
    <p class="text-base text-danger" role="alert">{failure}</p>
  {:else if composed}
    <p class="mb-2 text-xs text-fg-3 tabular">{t('changes.review.summary', {
        comments: tn('changes.review.comments', composed.commentCount),
        files: tn('changes.review.files', composed.fileCount),
        bytes: tn('changes.review.bytes', composed.bytes),
      })}</p>
    <!-- The prompt reads as it will in the chat; the quoted code shows as code blocks. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
    <div tabindex="0" aria-label={t('changes.send.prompt')} class="max-h-[40vh] overflow-auto rounded-md border border-line bg-surface-1 bg-clip-padding px-4 py-3">
      <Markdown source={composed.prompt} />
    </div>

    <p class="mt-5 mb-2 text-sm font-medium text-fg-2" id="send-to-label">{t('changes.send.to')}</p>
    <div class="flex flex-col gap-1" role="radiogroup" aria-labelledby="send-to-label">
      {#each options as option (option.value)}
        {@const selected = option.value === target}
        <button
          type="button"
          role="radio"
          aria-checked={selected}
          class={cx(
            'flex w-full items-center gap-3 rounded-md border px-3 py-2 text-left transition-colors duration-[var(--dur-fast)]',
            selected ? 'border-accent/30 bg-accent-wash' : 'border-transparent hover:bg-hover active:bg-press',
          )}
          onclick={() => (target = option.value)}
        >
          {#if option.fresh}<Plus size={16} aria-hidden="true" class="shrink-0 text-fg-3" />{:else}<MessageSquare size={16} aria-hidden="true" class="shrink-0 text-fg-3" />{/if}
          <span class="flex min-w-0 flex-1 flex-col">
            <span class="truncate text-sm font-medium text-fg">{option.title}</span>
            <span class="truncate text-xs text-fg-3">{option.detail}</span>
          </span>
          <Check size={16} aria-hidden="true" class={cx('shrink-0 text-accent-text', !selected && 'invisible')} />
        </button>
      {/each}
    </div>
  {:else}
    <div class="flex flex-col gap-2" role="status" aria-label={t('changes.send.composing')}>
      <Skeleton height="1rem" />
      <Skeleton height="1rem" />
      <Skeleton width="60%" height="1rem" />
    </div>
  {/if}

  {#snippet footer()}
    <Button variant="ghost" onclick={onclose}>{t('changes.send.cancel')}</Button>
    <Button variant="primary" loading={sending} disabled={!composed || options.length === 0} onclick={() => void send()}>{t('changes.send.submit')}</Button>
  {/snippet}
</Dialog>
