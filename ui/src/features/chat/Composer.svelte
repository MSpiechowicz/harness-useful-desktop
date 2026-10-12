<script lang="ts">
  import ArrowUp from '@lucide/svelte/icons/arrow-up';
  import Square from '@lucide/svelte/icons/square';
  import Terminal from '@lucide/svelte/icons/terminal';
  import { untrack } from 'svelte';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import { autofocus } from '$lib/a11y/autofocus';
  import { bashCommand, isBashDraft, sendIntent, type SendMode } from '$lib/chat/composer';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { acceptCommand, filterCommands, resolveSlash, slashQuery } from '$lib/chat/slash';
  import type { CommandSpec } from '$lib/ipc/bindings/CommandSpec';
  import { uid } from '$lib/utils/ids';
  import { isMacPlatform } from '$lib/utils/shortcuts';
  import { t } from '$lib/i18n/i18n.svelte';

  import ContextMeter from './ContextMeter.svelte';
  import ModelPicker from './ModelPicker.svelte';
  import ProjectPicker from './ProjectPicker.svelte';
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

  const placeholder = $derived(bash ? t('chat.composer.placeholder.bash') : session.streaming ? t('chat.composer.placeholder.streaming') : t('chat.composer.placeholder'));

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

<div class="chat-col relative pt-0 pb-5">
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

  <div
    class="border-card rounded-lg border-line-strong bg-surface-1 bg-clip-padding transition-[border-color] duration-[var(--dur-fast)] focus-within:border-[color-mix(in_oklch,var(--color-fg)_28%,var(--surface-1))]"
  >
    {#if bash}
      <p class="flex items-center gap-1.5 px-4 pt-3 text-xs text-warning"><Terminal size={14} aria-hidden="true" /> {t('chat.composer.bashHint')}</p>
    {/if}

    <textarea
      bind:this={textarea}
      bind:value={session.draft}
      use:autofocus
      {onkeydown}
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

    <div class="flex items-center gap-1 px-2 pb-2">
      <ModelPicker {session} />
      <ThinkingPicker {session} />

      <div class="ml-auto flex min-w-0 items-center gap-1.5">
        <ProjectPicker {session} />
        <ContextMeter usage={session.chat.session?.contextUsage ?? null} />

        {#if session.streaming}
          <Button size="sm" variant="ghost" disabled={!canSend} onclick={() => void submit('followUp')}>{t('chat.composer.followUp')}</Button>
          <Button size="sm" variant="secondary" disabled={!canSend} onclick={() => void submit('steer')}>{t('chat.composer.steer')}</Button>
          <Button size="sm" variant="danger" class="size-8 rounded-full! px-0!" aria-label={t('chat.composer.stop')} onclick={() => void session.abort()}>
            <Square size={12} aria-hidden="true" class="fill-current" />
          </Button>
        {:else}
          <Button size="sm" variant="primary" class="size-8 rounded-full! px-0!" aria-label={bash ? t('chat.composer.reviewCommand') : t('chat.composer.send')} disabled={!canSend} onclick={() => void submit('send')}>
            {#if bash}<Terminal size={16} aria-hidden="true" />{:else}<ArrowUp size={18} strokeWidth={2.25} aria-hidden="true" />{/if}
          </Button>
        {/if}
      </div>
    </div>
  </div>

  <p class="mt-2 truncate text-center text-xs text-fg-3">
    {#if session.streaming}
      {t('chat.composer.hint.streaming', { mod: isMacPlatform() ? '⌘' : 'Ctrl' })}
    {:else}
      {t('chat.composer.hint')}
    {/if}
  </p>
</div>
