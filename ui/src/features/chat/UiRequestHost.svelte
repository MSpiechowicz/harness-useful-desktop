<script lang="ts">
  import { onDestroy, untrack } from 'svelte';

  import type { SavedAnswers } from '$lib/chat/question-title';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import type { UiRequest } from '$lib/ipc/bindings/UiRequest';

  import QuestionsDialog from './QuestionsDialog.svelte';
  import UiRequestDialog from './UiRequestDialog.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  /** How long an answered question stays up for the next one before the dialog closes anyway. */
  const NEXT_QUESTION_WAIT_MS = 2000;

  // The agent sends the next question of a call only after it has the last answer. Closing the overlay in between would
  // fade the full-screen backdrop out and back in, so the answered question stays until the next one replaces it.
  let held = $state<UiRequest | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;
  // A sent answer that fails brings its dialog back: the user's answers come back with it.
  let saved = $state.raw<SavedAnswers | null>(null);

  const shown = $derived(session.dialog ?? held);

  function release(): void {
    clearTimeout(timer);
    held = null;
  }

  function hold(request: UiRequest): void {
    clearTimeout(timer);
    held = request;
    timer = setTimeout(release, NEXT_QUESTION_WAIT_MS);
  }

  $effect(() => {
    if (session.dialog) {
      untrack(release);
    }
  });

  // Another chat in this pane has nothing to do with the question being held.
  $effect(() => {
    void session;
    untrack(release);
  });

  onDestroy(() => clearTimeout(timer));
</script>

{#if shown}
  {@const request = shown}
  {#if request.kind.type === 'questions'}
    {#key request.id}
      <QuestionsDialog {session} {request} kind={request.kind} saved={saved?.id === request.id ? saved : null} onsave={(answers) => (saved = answers)} />
    {/key}
  {:else}
    <UiRequestDialog {session} {request} waiting={!session.dialog} onawaitnext={() => hold(request)} onrelease={release} />
  {/if}
{/if}
