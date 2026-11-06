<script lang="ts">
  import { tick, untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import type { UiRequest } from '$lib/ipc/bindings/UiRequest';
  import { autofocus } from '$lib/a11y/autofocus';
  import { questionTitle } from '$lib/chat/question-title';
  import { rovingIndex } from '$lib/utils/roving';
  import { t } from '$lib/i18n/i18n.svelte';
  import { visibleText } from '$lib/utils/display-text';

  import Countdown from './Countdown.svelte';
  import QuestionOption from './QuestionOption.svelte';
  import QuestionProgress from './QuestionProgress.svelte';

  interface Props {
    session: ChatSession;
    request: UiRequest;
    /** This request is answered and the next question of its call is on its way. */
    waiting?: boolean;
    /** A question with more to come was answered: the dialog stays open for the next one. */
    onawaitnext?: () => void;
    /** Closed while waiting, when there is nothing to answer. */
    onrelease?: () => void;
  }

  let { session, request, waiting = false, onawaitnext, onrelease }: Props = $props();

  // Questions sent all at once have their own dialog (`QuestionsDialog`).
  const kind = $derived(request.kind as Exclude<UiRequest['kind'], { type: 'questions' }>);
  const timeoutMs = $derived(kind.type === 'editor' ? null : kind.timeoutMs);

  // The step of a question with several parts is shown as a track, not as part of its title.
  const heading = $derived(request.purpose === 'question' ? questionTitle(kind.title) : { text: kind.title, step: null });
  const title = $derived(visibleText(heading.text));

  let text = $state('');
  let optionButtons: (HTMLButtonElement | undefined)[] = $state([]);

  // Editors and inputs start from what the agent suggested.
  $effect(() => {
    text = request.kind.type === 'editor' ? (request.kind.prefill ?? '') : '';
  });

  // The dialog stays open from one question to the next, so the first option of a new one takes the focus here.
  let focusedRequest = untrack(() => request.id);
  // Focus waits here rather than on an option, whose focus ring would look like a choice already made.
  let start: HTMLElement | undefined = $state();

  $effect(() => {
    if (request.id !== focusedRequest) {
      focusedRequest = request.id;
      void tick().then(() => start?.focus());
    }
  });

  function cancel(): void {
    if (waiting) {
      onrelease?.();

      return;
    }

    void session.respond(request.id, { type: 'cancelled' });
  }

  function answer(value: string): void {
    if (waiting) {
      return;
    }

    if (heading.step && heading.step.number < heading.step.total) {
      onawaitnext?.();
    }

    void session.respond(request.id, { type: 'value', value });
  }

  function pick(option: string): void {
    answer(option);
  }

  function submitText(): void {
    answer(text);
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

<!-- A stray click must not answer for the user: an agent dialog closes through its buttons only. -->
<Dialog
  open
  {title}
  description={request.purpose === 'question' ? t('chat.request.agentAsks') : undefined}
  size={kind.type === 'editor' ? 'lg' : 'md'}
  dismissible={false}
  initialFocus={kind.type === 'select' ? '[data-focus-start]' : undefined}
>
  <span bind:this={start} tabindex="-1" class="sr-only" data-focus-start></span>

  {#key request.id}

    {#if heading.step}
      <QuestionProgress number={heading.step.number} total={heading.step.total} />
    {/if}

    {#if timeoutMs}
      <div class="mb-3 flex items-center gap-2 text-sm text-fg-3">{t('chat.request.closes')} <Countdown {timeoutMs} requestId={request.id} ontimeout={cancel} /></div>
    {/if}

    {#if kind.type === 'select'}
      {@const options = kind.options}
      <ul class="flex flex-col gap-3" aria-label={title}>
        {#each options as option, index (index)}
          <li>
            <QuestionOption
              bind:button={optionButtons[index]}
              number={index + 1}
              label={option}
              detail={kind.details[index]}
              onclick={() => pick(option)}
              onkeydown={(event) => onOptionKeydown(event, options, index)}
            />
          </li>
        {/each}
      </ul>
    {:else if kind.type === 'confirm'}
      <p class="text-base break-words whitespace-pre-wrap text-fg-2">{visibleText(kind.message)}</p>
    {:else if kind.type === 'input'}
      <input
        bind:value={text}
        use:autofocus
        aria-label={title}
        type={request.secret ? 'password' : 'text'}
        autocomplete="off"
        placeholder={visibleText(kind.placeholder ?? '')}
        onkeydown={(event) => event.key === 'Enter' && submitText()}
        class="h-control w-full rounded-md border border-line-strong bg-surface-2 px-3 text-base text-fg outline-none focus:border-focus"
      />
    {:else}
      <textarea
        bind:value={text}
        use:autofocus
        aria-label={title}
        rows="12"
        spellcheck="false"
        onkeydown={onEditorKeydown}
        class="w-full resize-none rounded-md border border-line-strong bg-surface-2 px-3 py-2 font-mono text-sm text-fg outline-none focus:border-focus"
      ></textarea>
    {/if}
  {/key}

  {#snippet footer()}
    {#if kind.type === 'confirm'}
      <Button onclick={() => void session.respond(request.id, { type: 'confirmed', confirmed: false })}>{t('chat.request.no')}</Button>
      <Button variant="primary" onclick={() => void session.respond(request.id, { type: 'confirmed', confirmed: true })}>{t('chat.request.yes')}</Button>
    {:else if kind.type === 'input' || kind.type === 'editor'}
      <Button onclick={cancel}>{t('chat.request.cancel')}</Button>
      <Button variant="primary" onclick={submitText}>{t('chat.request.submit')}</Button>
    {:else}
      <Button onclick={cancel}>{t('chat.request.cancel')}</Button>
    {/if}
  {/snippet}
</Dialog>
