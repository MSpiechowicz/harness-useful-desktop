<script lang="ts">
  import CornerDownLeft from '@lucide/svelte/icons/corner-down-left';
  import Square from '@lucide/svelte/icons/square';
  import Terminal from '@lucide/svelte/icons/terminal';
  import { untrack } from 'svelte';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import Kbd from '$lib/components/Kbd.svelte';
  import { autofocus } from '$lib/a11y/autofocus';
  import { bashCommand, isBashDraft, sendIntent, type SendMode } from '$lib/chat/composer';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { acceptCommand, filterCommands, resolveSlash, slashQuery } from '$lib/chat/slash';
  import type { CommandSpec } from '$lib/ipc/bindings/CommandSpec';
  import { uid } from '$lib/utils/ids';

  import { runSlash } from './slash-run';
  import SlashMenu from './SlashMenu.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  const MAX_HEIGHT_PX = 220;
  const menuId = uid('slash-menu');

  let textarea: HTMLTextAreaElement | undefined = $state();
  let active = $state(0);
  /** The draft for which Escape closed the suggestions; typing brings them back. */
  let dismissedFor = $state<string | null>(null);
  let appliedNonce = untrack(() => session.chat.editorText?.nonce ?? 0);

  const query = $derived(slashQuery(session.draft));
  const suggestions = $derived(query === null ? [] : filterCommands(session.commands, query));
  const menuOpen = $derived(suggestions.length > 0 && dismissedFor !== session.draft && !session.ended);
  const bash = $derived(isBashDraft(session.draft));
  const canSend = $derived(session.draft.trim() !== '' && !session.ended);
  const queued = $derived(session.chat.queue.steering.length + session.chat.queue.followUp.length);

  const placeholder = $derived(bash ? 'Shell command to run in the project…' : session.streaming ? 'Steer the agent, or queue a follow-up…' : 'Message the agent… (/ for commands, ! for a shell command)');

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
    if (text === '' || session.ended) {
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
    session.draft = '';
    if (!(await session.submit(text, mode)) && session.draft === '') {
      session.draft = text;
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

    const intent = sendIntent(event, session.streaming);
    if (intent) {
      event.preventDefault();
      void submit(intent);
    }
  }
</script>

<div class="chat-col relative pt-2 pb-4">
  {#if menuOpen}
    <div class="absolute inset-x-[var(--gutter)] top-0">
      <SlashMenu id={menuId} commands={suggestions} query={query ?? ''} {active} onaccept={accept} onhover={(index) => (active = index)} />
    </div>
  {/if}

  {#if queued > 0}
    <ul class="mb-2 flex flex-wrap gap-1.5" aria-label="Queued messages">
      {#each session.chat.queue.steering as message, index (`s${index}`)}
        <li><Badge tone="accent">Steering: {message.slice(0, 48)}</Badge></li>
      {/each}
      {#each session.chat.queue.followUp as message, index (`f${index}`)}
        <li><Badge tone="info">Next: {message.slice(0, 48)}</Badge></li>
      {/each}
    </ul>
  {/if}

  <div class="rounded-xl border border-line-strong bg-surface-1/80 shadow-float transition-colors focus-within:border-focus">
    {#if bash}
      <p class="flex items-center gap-1.5 px-3.5 pt-2.5 text-xs text-warning"><Terminal size={16} aria-hidden="true" /> Shell command · you will confirm it before it runs</p>
    {/if}

    <textarea
      bind:this={textarea}
      bind:value={session.draft}
      use:autofocus
      {onkeydown}
      rows="1"
      disabled={session.ended}
      aria-label="Message"
      role="combobox"
      aria-expanded={menuOpen}
      aria-controls={menuOpen ? menuId : undefined}
      aria-activedescendant={menuOpen ? `${menuId}-${active}` : undefined}
      aria-autocomplete="list"
      spellcheck="true"
      {placeholder}
      class="block max-h-56 w-full resize-none bg-transparent px-3.5 py-3 text-base text-fg outline-none placeholder:text-fg-3 disabled:cursor-not-allowed disabled:opacity-60 {bash ? 'font-mono' : ''}"
    ></textarea>

    <div class="flex items-center gap-2 px-2.5 pb-2.5">
      <p class="min-w-0 flex-1 truncate text-xs text-fg-3">
        {#if session.streaming}
          <Kbd keys="enter" /> steer · <Kbd keys="mod+enter" /> follow-up
        {:else}
          <Kbd keys="enter" /> send · <Kbd keys="shift+enter" /> new line
        {/if}
      </p>

      {#if session.streaming}
        <Button size="sm" variant="danger" onclick={() => void session.abort()}>
          {#snippet leading()}<Square size={16} aria-hidden="true" />{/snippet}
          Stop
        </Button>
        <Button size="sm" disabled={!canSend} onclick={() => void submit('followUp')}>Follow-up</Button>
        <Button size="sm" variant="primary" disabled={!canSend} onclick={() => void submit('steer')}>Steer</Button>
      {:else}
        <Button size="sm" variant="primary" disabled={!canSend} onclick={() => void submit('send')}>
          {#snippet trailing()}<CornerDownLeft size={16} aria-hidden="true" />{/snippet}
          {bash ? 'Review command' : 'Send'}
        </Button>
      {/if}
    </div>
  </div>
</div>
