<script lang="ts">
  import ArrowUp from '@lucide/svelte/icons/arrow-up';
  import Paperclip from '@lucide/svelte/icons/paperclip';
  import Terminal from '@lucide/svelte/icons/terminal';
  import { onDestroy, onMount, untrack } from 'svelte';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import { cx } from '$lib/utils/cx';
  import { autofocus } from '$lib/a11y/autofocus';
  import { MAX_FILES, MAX_IMAGES, isLargePaste } from '$lib/chat/attachments';
  import { bashCommand, escapeStep, isBashDraft, sendIntent, STOP_WINDOW_MS, type BusyMode, type SendMode } from '$lib/chat/composer';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { acceptCommand, filterCommands, resolveSlash, slashQuery } from '$lib/chat/slash';
  import type { CommandSpec } from '$lib/ipc/bindings/CommandSpec';
  import { uid } from '$lib/utils/ids';
  import { t } from '$lib/i18n/i18n.svelte';

  import AttachmentStrip from './AttachmentStrip.svelte';
  import ComposerFooter from './ComposerFooter.svelte';
  import ComposerMode from './ComposerMode.svelte';
  import ModelPicker from './ModelPicker.svelte';
  import { runSlash } from './slash-run';
  import SlashMenu from './SlashMenu.svelte';
  import ThinkingPicker from './ThinkingPicker.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  const MAX_HEIGHT_PX = 220;
  const menuId = uid('slash-menu');

  let textarea: HTMLTextAreaElement | undefined = $state();
  let fileInput: HTMLInputElement | undefined = $state();
  let dragging = $state(false);
  let active = $state(0);
  /** The draft for which Escape closed the suggestions; typing brings them back. */
  let dismissedFor = $state<string | null>(null);
  /** What Enter does while the agent works; always Steer again once it has finished. */
  let whenBusy = $state<BusyMode>('steer');
  /** The time of the first Escape, while the stop waits for a second one. */
  let stopArmedAt = $state<number | null>(null);
  let stopTimer: ReturnType<typeof setTimeout> | undefined;
  let appliedNonce = untrack(() => session.chat.editorText?.nonce ?? 0);

  // Files dragged in from the file manager arrive as native drops (see `drop_listen`); the HTML5
  // handlers below cover browsers and pasted files.
  onMount(() =>
    session.listenForDrops((active) => {
      dragging = active;
      if (!active) {
        textarea?.focus();
      }
    }),
  );

  const query = $derived(slashQuery(session.draft));
  const suggestions = $derived(query === null ? [] : filterCommands(session.commands, query));
  const menuOpen = $derived(suggestions.length > 0 && dismissedFor !== session.draft && !session.ended);
  const bash = $derived(isBashDraft(session.draft));
  const attached = $derived(session.attached);
  const canSend = $derived((session.draft.trim() !== '' || attached.files.length > 0) && !session.ended);
  const attachFull = $derived(attached.images.length >= MAX_IMAGES && attached.files.length >= MAX_FILES);
  const stopArmed = $derived(stopArmedAt !== null);
  /** While the agent works and nothing is typed, the round button stops it. */
  const stopping = $derived(session.streaming && !canSend);
  const queued = $derived(session.chat.queue.steering.length + session.chat.queue.followUp.length);

  const placeholder = $derived(bash ? t('chat.composer.placeholder.bash') : session.streaming ? t('chat.composer.placeholder.streaming') : t('chat.composer.placeholder'));

  $effect(() => {
    if (!session.streaming) {
      whenBusy = 'steer';
      disarmStop();
    }
  });

  onDestroy(() => clearTimeout(stopTimer));

  function disarmStop(): void {
    clearTimeout(stopTimer);
    stopArmedAt = null;
  }

  // Escape twice stops the agent; one Escape alone is too easy to hit. Menus and dialogs take their own Escape first.
  function onwindowkeydown(event: KeyboardEvent): void {
    if (event.key !== 'Escape' || event.defaultPrevented || event.repeat || event.isComposing || !session.streaming) {
      return;
    }

    if (escapeStep(stopArmedAt, Date.now()) === 'stop') {
      disarmStop();
      void session.abort();

      return;
    }

    clearTimeout(stopTimer);
    stopArmedAt = Date.now();
    stopTimer = setTimeout(disarmStop, STOP_WINDOW_MS);
  }

  $effect(() => {
    void suggestions;
    active = 0;
  });

  // Grow with the text, up to a cap.
  $effect(() => {
    void session.draft;
    if (textarea) {
      textarea.style.height = 'auto';
      textarea.style.height = `${Math.min(textarea.scrollHeight, MAX_HEIGHT_PX)}px`;
    }
  });

  // The agent can ask for text to be put in the composer.
  $effect(() => {
    const request = session.chat.editorText;
    if (request && request.nonce !== appliedNonce) {
      appliedNonce = request.nonce;
      session.draft = request.text;
      textarea?.focus();
    }
  });

  function attach(files: Iterable<File>): void {
    const list = Array.from(files);
    if (list.length > 0 && !session.ended && !bash) {
      void attached.add(list);
    }
  }

  function onpaste(event: ClipboardEvent): void {
    const files = Array.from(event.clipboardData?.files ?? []);
    if (files.length > 0) {
      event.preventDefault();
      attach(files);

      return;
    }

    // A pasted log would bury the composer: it goes along as a text file instead.
    const text = event.clipboardData?.getData('text/plain') ?? '';
    if (isLargePaste(text) && !bash && !session.ended) {
      event.preventDefault();
      void attached.addText(text);
    }
  }

  function hasFiles(event: DragEvent): boolean {
    return event.dataTransfer?.types.includes('Files') ?? false;
  }

  function ondragover(event: DragEvent): void {
    if (hasFiles(event) && !session.ended) {
      event.preventDefault();
      dragging = true;
    }
  }

  function ondrop(event: DragEvent): void {
    dragging = false;
    if (hasFiles(event)) {
      event.preventDefault();
      attach(event.dataTransfer?.files ?? []);
      textarea?.focus();
    }
  }

  function onpick(): void {
    attach(fileInput?.files ?? []);
    if (fileInput) {
      fileInput.value = '';
    }
  }

  function accept(command: CommandSpec): void {
    const outcome = acceptCommand(command);

    if (outcome.type !== 'insert') {
      session.draft = '';
    }

    void runSlash(session, outcome);
    textarea?.focus();
  }

  async function submit(mode: SendMode): Promise<void> {
    const text = session.draft.trim();
    if ((text === '' && attached.files.length === 0) || session.ended) {
      return;
    }

    // A shell command never goes out without the user's explicit confirmation.
    if (isBashDraft(text)) {
      const command = bashCommand(text);
      if (command) {
        session.ui.confirmBash = command;
      }

      return;
    }

    if (text.startsWith('/')) {
      const outcome = resolveSlash(text, session.commands);
      if (outcome.type !== 'send') {
        session.draft = '';
        await runSlash(session, outcome);

        return;
      }
    }

    // Clear at once so the composer feels instant; a rejected message comes back.
    const sent = attached.take();
    session.draft = '';
    if (!(await session.submit(text, mode, sent)) && session.draft === '') {
      session.draft = text;
      attached.restore(sent);
    }
  }

  function onkeydown(event: KeyboardEvent): void {
    if (menuOpen) {
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault();
        const step = event.key === 'ArrowDown' ? 1 : -1;
        active = (active + step + suggestions.length) % suggestions.length;
      } else if ((event.key === 'Enter' && !event.shiftKey && !event.isComposing) || event.key === 'Tab') {
        event.preventDefault();
        accept(suggestions[active]);
      } else if (event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        dismissedFor = session.draft;
      }

      return;
    }

    const intent = sendIntent(event, session.streaming, whenBusy);
    if (intent) {
      event.preventDefault();
      void submit(intent);
    }
  }
</script>

<svelte:window onkeydown={onwindowkeydown} />

<!-- 72px puts the card's bottom edge level with the top edge of the rail footer's language picker, which is anchored to the
     window bottom, so this holds with or without an update card above it. -->
<div class="chat-col relative pt-0 pb-[72px]">
  {#if menuOpen}
    <div class="absolute inset-x-[var(--gutter)] top-0">
      <SlashMenu id={menuId} commands={suggestions} query={query ?? ''} {active} onaccept={accept} onhover={(index) => (active = index)} />
    </div>
  {/if}

  {#if queued > 0}
    <ul class="mb-2 flex flex-wrap gap-1.5" aria-label={t('chat.composer.queued')}>
      {#each session.chat.queue.steering as message, index (`s${index}`)}
        <li><Badge tone="accent">{t('chat.composer.steering', { text: message.slice(0, 48) })}</Badge></li>
      {/each}
      {#each session.chat.queue.followUp as message, index (`f${index}`)}
        <li><Badge tone="info">{t('chat.composer.next', { text: message.slice(0, 48) })}</Badge></li>
      {/each}
    </ul>
  {/if}

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    {ondragover}
    ondragleave={() => (dragging = false)}
    {ondrop}
    class="@container border-card chat-radius border-line-strong bg-surface-1 bg-clip-padding transition-[border-color] duration-[var(--dur-fast)] focus-within:border-[color-mix(in_oklch,var(--color-fg)_28%,var(--surface-1))] {dragging ? 'border-accent!' : ''}"
  >
    <AttachmentStrip {session} />

    {#if bash}
      <p class="flex items-center gap-1.5 px-4 pt-3 text-xs text-warning"><Terminal size={14} aria-hidden="true" /> {t('chat.composer.bashHint')}</p>
    {/if}

    <textarea
      bind:this={textarea}
      bind:value={session.draft}
      use:autofocus
      {onkeydown}
      {onpaste}
      rows="1"
      disabled={session.ended}
      aria-label={t('chat.composer.aria')}
      role="combobox"
      aria-expanded={menuOpen}
      aria-controls={menuOpen ? menuId : undefined}
      aria-activedescendant={menuOpen ? `${menuId}-${active}` : undefined}
      aria-autocomplete="list"
      spellcheck="true"
      {placeholder}
      class="block max-h-56 min-h-14 w-full resize-none bg-transparent px-4 pt-3.5 pb-1 text-[15px] leading-6 text-fg outline-none placeholder:text-fg-3 disabled:cursor-not-allowed disabled:opacity-60 {bash ? 'font-mono text-sm' : ''}"
    ></textarea>

    <!-- Left: 8px + the button's own padding puts the paperclip about 16px from the edge; the send button gets the same 16px. -->
    <div class="flex items-center gap-1 pr-4 pb-2 pl-2">
      <input bind:this={fileInput} type="file" multiple class="hidden" tabindex="-1" aria-hidden="true" onchange={onpick} />
      <button
        type="button"
        aria-label={t('chat.attach.add')}
        title={t('chat.attach.add')}
        disabled={session.ended || bash || attachFull}
        onclick={() => fileInput?.click()}
        class="inline-flex h-control items-center justify-center gap-1.5 rounded-md pr-2 pl-1.5 text-sm text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:bg-transparent disabled:hover:text-fg-3 @max-[560px]:w-control @max-[560px]:px-0"
      >
        <Paperclip size={16} aria-hidden="true" />
        <span class="@max-[560px]:hidden">{t('chat.composer.attach')}</span>
      </button>
      <ModelPicker {session} />
      <ThinkingPicker {session} />

      <div class="ml-auto flex shrink-0 items-center gap-1.5">
        {#if stopArmed}
          <span class="text-xs text-danger @max-[460px]:hidden" role="status">{t('chat.composer.stopArmed')}</span>
        {/if}

        {#if session.streaming && !stopping}
          <ComposerMode bind:value={whenBusy} />
        {/if}

        {#if stopping}
          <Button
            variant="danger"
            class={cx('size-control px-0!', stopArmed && 'animate-pulse')}
            aria-label={t('chat.composer.stop')}
            title={t('chat.composer.stopHint')}
            onclick={() => void session.abort()}
          >
            <svg viewBox="0 0 24 24" class="size-[18px]" aria-hidden="true"><rect x="4" y="4" width="16" height="16" rx="3.5" fill="currentColor" /></svg>
          </Button>
        {:else}
          <Button
            variant="primary"
            class="size-control px-0!"
            aria-label={bash ? t('chat.composer.reviewCommand') : t('chat.composer.send')}
            disabled={!canSend}
            onclick={() => void submit(session.streaming ? whenBusy : 'send')}
          >
            {#if bash}<Terminal size={18} aria-hidden="true" />{:else}<ArrowUp size={20} strokeWidth={2.5} aria-hidden="true" />{/if}
          </Button>
        {/if}
      </div>
    </div>

    <ComposerFooter {session} />
  </div>
</div>
