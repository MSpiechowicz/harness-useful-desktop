<script lang="ts">
  import ListChecks from '@lucide/svelte/icons/list-checks';
  import { tick, untrack } from 'svelte';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import type { UiRequest } from '$lib/ipc/bindings/UiRequest';
  import { autofocus } from '$lib/a11y/autofocus';
  import { questionText, type Draft, type SavedAnswers } from '$lib/chat/question-title';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';
  import { visibleText } from '$lib/utils/display-text';
  import { rovingIndex } from '$lib/utils/roving';

  import Countdown from './Countdown.svelte';
  import QuestionOption from './QuestionOption.svelte';
  import QuestionProgress from './QuestionProgress.svelte';

  type QuestionsKind = Extract<UiRequest['kind'], { type: 'questions' }>;

  interface Props {
    session: ChatSession;
    request: UiRequest;
    kind: QuestionsKind;
    /** The answers of an earlier try at this request, when it came back. */
    saved?: SavedAnswers | null;
    /** The answers are on their way: kept in case the request comes back. */
    onsave?: (saved: SavedAnswers) => void;
  }

  let { session, request, kind, saved = null, onsave }: Props = $props();

  /** How long before the agent's own deadline the answers so far are sent, so they arrive before it gives up. */
  const SEND_BEFORE_DEADLINE_MS = 1500;

  // What the backend reads as a whole number: no exponent or decimals, and small enough for 64 bits.
  const WHOLE_NUMBER = /^[+-]?\d{1,18}$/;

  // Nothing reaches the agent before the last question is submitted, so any answer can still change.
  let step = $state(untrack(() => saved?.step ?? 0));
  let drafts = $state<Draft[]>(untrack(() => saved?.drafts ?? kind.questions.map((item) => ({ selected: [], custom: '', typing: item.options.length === 0 }))));
  let optionButtons: (HTMLButtonElement | undefined)[] = $state([]);
  // Focus waits here rather than on an option, whose focus ring would look like a choice already made.
  let start: HTMLElement | undefined = $state();

  const total = $derived(kind.questions.length);
  const question = $derived(kind.questions[step]);
  const text = $derived(questionText(question.question));
  const draft = $derived(drafts[step]);
  const last = $derived(step === total - 1);
  // A question with no options only takes typed text. Typed text can be left out unless the question offers options to pick.
  const free = $derived(question.options.length === 0);
  const offersOwn = $derived(!free && question.custom);
  const rowCount = $derived(question.options.length + (offersOwn ? 1 : 0));
  const typed = $derived(draft.typing && draft.custom.trim() !== '');
  const validNumber = $derived(
    !typed || question.input === 'text' || (question.input === 'integer' ? WHOLE_NUMBER.test(draft.custom.trim()) : Number.isFinite(Number(draft.custom))),
  );
  const answered = $derived(validNumber && (free || question.multi || draft.selected.length > 0 || typed));
  // Choosing Other counts as soon as it is clicked, before anything is typed.
  const pickedAny = $derived(draft.selected.length > 0 || draft.typing);

  async function goTo(next: number): Promise<void> {
    step = next;
    await tick();

    if (!drafts[next].typing) {
      start?.focus();
    }
  }

  function pick(option: number): void {
    if (question.multi) {
      draft.selected = draft.selected.includes(option) ? draft.selected.filter((item) => item !== option) : [...draft.selected, option];

      return;
    }

    draft.selected = [option];
    draft.typing = false;
  }

  function typeOwn(): void {
    if (!question.multi) {
      draft.selected = [];
    }

    draft.typing = question.multi ? !draft.typing : true;
  }

  function submit(): void {
    const answers = kind.questions.map((item, index) => {
      const own = drafts[index].typing ? drafts[index].custom.trim() : '';
      const labels = drafts[index].selected.map((option) => item.options[option].label);

      // Two rows can share a label, and a label can only be sent once.
      return { id: item.id, selectedOptions: [...new Set(labels)], customInput: own === '' ? null : own };
    });

    onsave?.({ id: request.id, step, drafts: $state.snapshot(drafts) });
    void session.respond(request.id, { type: 'answers', answers });
  }

  // Without this the agent's deadline would throw away what was picked and answer for the user.
  $effect(() => {
    if (!kind.timeoutMs) {
      return;
    }

    const delay = Math.max(kind.timeoutMs - SEND_BEFORE_DEADLINE_MS, kind.timeoutMs / 2);
    const timer = setTimeout(() => {
      if (drafts.some((item) => item.selected.length > 0 || item.custom.trim() !== '')) {
        submit();
      }
    }, delay);

    return () => clearTimeout(timer);
  });

  function forward(): void {
    if (!answered) {
      return;
    }

    if (last) {
      submit();
    } else {
      void goTo(step + 1);
    }
  }

  function cancel(): void {
    void session.respond(request.id, { type: 'cancelled' });
  }

  /** From the resting place the arrow keys enter the options and the digits still pick one. */
  function onWindowKeydown(event: KeyboardEvent): void {
    if (event.target !== start) {
      return;
    }

    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      optionButtons[event.key === 'ArrowDown' ? 0 : rowCount - 1]?.focus();

      return;
    }

    onOptionKeydown(event, 0);
  }

  /** Arrow keys move through the options; the digits 1-9 pick one directly. */
  function onOptionKeydown(event: KeyboardEvent, index: number): void {
    const count = rowCount;
    const digit = Number(event.key);

    if (digit >= 1 && digit <= Math.min(9, question.options.length)) {
      event.preventDefault();
      pick(digit - 1);

      return;
    }

    const next = rovingIndex(event.key, index, count, 'vertical');

    if (next !== null) {
      event.preventDefault();
      optionButtons[next]?.focus();
    }
  }
</script>

<svelte:window onkeydown={onWindowKeydown} />

<Dialog
  open
  title={visibleText(text.title)}
  description={t('chat.request.agentAsks')}
  dismissible={false}
  initialFocus={draft.typing ? 'input' : '[data-focus-start]'}
>
  <span bind:this={start} tabindex="-1" class="sr-only" data-focus-start></span>

  {#if kind.timeoutMs}
    <div class="mb-3 flex items-center gap-2 text-sm text-fg-3">{t('chat.request.closesSending')} <Countdown timeoutMs={kind.timeoutMs} requestId={request.id} ontimeout={cancel} /></div>
  {/if}

  <!-- Only a ratchet approval's stage says something new: any other header repeats its question in fewer words. -->
  {#if question.header?.startsWith('ratchet · ')}
    <Badge class="mb-3">{visibleText(question.header)}</Badge>
  {/if}

  {#if total > 1}
    <QuestionProgress number={step + 1} {total} />
  {/if}

  {#if text.body}
    <p class="mb-4 text-sm break-words whitespace-pre-wrap text-fg-2" data-question-body>{visibleText(text.body)}</p>
  {/if}

  {#if question.multi}
    <p class="mb-3 flex items-center gap-2 text-base font-medium text-fg"><ListChecks class="size-4 shrink-0 text-accent-text" aria-hidden="true" />{t('chat.request.pickAny')}</p>
  {/if}

  {#key step}
    {#if !free}
    <ul class="flex flex-col gap-3" aria-label={visibleText(text.title)}>
      {#each question.options as option, index (index)}
        <li>
          <QuestionOption
            bind:button={optionButtons[index]}
            number={index + 1}
            label={option.label}
            detail={option.description}
            recommended={question.recommended === index}
            selected={draft.selected.includes(index)}
            otherPicked={pickedAny && !draft.selected.includes(index)}
            onclick={() => pick(index)}
            onkeydown={(event) => onOptionKeydown(event, index)}
          />
        </li>
      {/each}

      {#if offersOwn}
      <li>
        <QuestionOption
          bind:button={optionButtons[question.options.length]}
          number={question.options.length + 1}
          label={t('chat.request.ownAnswer')}
          selected={draft.typing}
          onclick={typeOwn}
          onkeydown={(event) => onOptionKeydown(event, question.options.length)}
        />
      </li>
      {/if}
    </ul>
    {/if}

    {#if draft.typing}
      <input
        value={draft.custom}
        oninput={(event) => (draft.custom = event.currentTarget.value)}
        use:autofocus
        aria-label={free ? visibleText(text.title) : t('chat.request.ownAnswer')}
        type={question.input === 'text' ? 'text' : 'number'}
        step={question.input === 'integer' ? 1 : undefined}
        autocomplete="off"
        onkeydown={(event) => event.key === 'Enter' && forward()}
        class={cx('h-control w-full rounded-md border border-line-strong bg-surface-2 px-3 text-base text-fg outline-none', !free && 'mt-3')}
      />
    {/if}
  {/key}

  {#snippet footer()}
    <Button variant="danger" class="mr-auto" onclick={cancel}>{t('chat.request.cancel')}</Button>
    {#if step > 0}
      <Button onclick={() => void goTo(step - 1)}>{t('chat.request.back')}</Button>
    {/if}
    <Button variant="primary" disabled={!answered} title={answered ? undefined : t('chat.request.pickFirst')} onclick={forward}>
      {t(last ? 'chat.request.submit' : 'chat.request.next')}
    </Button>
  {/snippet}
</Dialog>
