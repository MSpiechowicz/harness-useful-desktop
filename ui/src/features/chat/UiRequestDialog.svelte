<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import type { UiRequest } from '$lib/ipc/bindings/UiRequest';
  import { autofocus } from '$lib/a11y/autofocus';
  import { rovingIndex } from '$lib/utils/roving';

  import Countdown from './Countdown.svelte';

  interface Props {
    session: ChatSession;
    request: UiRequest;
  }

  let { session, request }: Props = $props();

  const kind = $derived(request.kind);
  const timeoutMs = $derived(kind.type === 'editor' ? null : kind.timeoutMs);

  let text = $state('');
  let optionButtons: (HTMLButtonElement | undefined)[] = $state([]);

  // Editors and inputs start from what the agent suggested.
  $effect(() => {
    text = request.kind.type === 'editor' ? (request.kind.prefill ?? '') : '';
  });

  function cancel(): void {
    void session.respond(request.id, { type: 'cancelled' });
  }

  function pick(option: string): void {
    void session.respond(request.id, { type: 'value', value: option });
  }

  function submitText(): void {
    void session.respond(request.id, { type: 'value', value: text });
  }

  /** Arrow keys move through the options; the digits 1-9 pick one directly. */
  function onOptionKeydown(event: KeyboardEvent, options: string[], index: number): void {
    const digit = Number(event.key);
    if (digit >= 1 && digit <= Math.min(9, options.length)) {
      event.preventDefault();
      pick(options[digit - 1]);

      return;
    }

    const next = rovingIndex(event.key, index, options.length, 'vertical');
    if (next !== null) {
      event.preventDefault();
      optionButtons[next]?.focus();
    }
  }

  function onEditorKeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      submitText();
    }
  }
</script>

<Dialog open title={kind.title} size={kind.type === 'editor' ? 'lg' : 'md'} onclose={cancel}>
  {#if timeoutMs}
    <div class="mb-3 flex items-center gap-2 text-sm text-fg-3">Closes by itself when the time runs out <Countdown {timeoutMs} requestId={request.id} ontimeout={cancel} /></div>
  {/if}

  {#if kind.type === 'select'}
    {@const options = kind.options}
    <ul class="flex flex-col gap-1.5" aria-label={kind.title}>
      {#each options as option, index (index)}
        <li>
          <button
            bind:this={optionButtons[index]}
            type="button"
            class="flex w-full flex-col items-start gap-0.5 rounded-md border border-line bg-surface-2 px-3.5 py-2.5 text-left transition-colors hover:bg-surface-3"
            onclick={() => pick(option)}
            onkeydown={(event) => onOptionKeydown(event, options, index)}
          >
            <span class="text-base text-fg"><span class="mr-2 text-fg-3">{index + 1}</span>{option}</span>
            {#if kind.details[index]}<span class="text-sm text-fg-2">{kind.details[index]}</span>{/if}
          </button>
        </li>
      {/each}
    </ul>
  {:else if kind.type === 'confirm'}
    <p class="text-base break-words whitespace-pre-wrap text-fg-2">{kind.message}</p>
  {:else if kind.type === 'input'}
    <input
      bind:value={text}
      use:autofocus
      aria-label={kind.title}
      placeholder={kind.placeholder ?? ''}
      onkeydown={(event) => event.key === 'Enter' && submitText()}
      class="h-10 w-full rounded-md border border-line-strong bg-surface-2 px-3 text-base text-fg outline-none focus:border-focus"
    />
  {:else}
    <textarea
      bind:value={text}
      use:autofocus
      aria-label={kind.title}
      rows="12"
      spellcheck="false"
      onkeydown={onEditorKeydown}
      class="w-full resize-none rounded-md border border-line-strong bg-surface-2 px-3 py-2 font-mono text-sm text-fg outline-none focus:border-focus"
    ></textarea>
  {/if}

  {#snippet footer()}
    {#if kind.type === 'confirm'}
      <Button onclick={() => void session.respond(request.id, { type: 'confirmed', confirmed: false })}>No</Button>
      <Button variant="primary" onclick={() => void session.respond(request.id, { type: 'confirmed', confirmed: true })}>Yes</Button>
    {:else if kind.type === 'input' || kind.type === 'editor'}
      <Button onclick={cancel}>Cancel</Button>
      <Button variant="primary" onclick={submitText}>Submit</Button>
    {:else}
      <Button onclick={cancel}>Cancel</Button>
    {/if}
  {/snippet}
</Dialog>
