<script lang="ts">
  import ShieldAlert from '@lucide/svelte/icons/shield-alert';

  import Button from '$lib/components/Button.svelte';
  import Kbd from '$lib/components/Kbd.svelte';
  import { approvalSubject, type ApprovalPrompt } from '$lib/chat/approval';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { commandOf, fullPrimaryArg, TOOL_ICONS, toolKind, toolTitle } from '$lib/chat/tool-meta';
  import { t, type MessageKey } from '$lib/i18n/i18n.svelte';
  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
  import { overflowing } from '$lib/utils/overflowing';

  import Countdown from './Countdown.svelte';
  import TerminalBlock from './TerminalBlock.svelte';

  interface Props {
    session: ChatSession;
    prompt: ApprovalPrompt;
  }

  let { session, prompt }: Props = $props();

  /** Harnesses whose card intent is their own reading of a command, a hint that may leave parts out: it says whose reading it is. */
  const READINGS: Partial<Record<HarnessId, MessageKey>> = { codex: 'chat.approvalCard.readsAs.codex' };

  const subject = $derived(approvalSubject(session.chat, prompt));
  const Icon = $derived(subject ? TOOL_ICONS[toolKind(subject.name)] : ShieldAlert);
  const command = $derived(subject ? commandOf(subject) : null);
  // Nothing about what is approved is cut off: the command and the argument are shown in full.
  const argument = $derived(subject ? fullPrimaryArg(subject.args) : null);
  // A Codex ask can be about something else than the command of its card (input for it, or one command it runs): the title
  // then names what is asked, which the command below does not show.
  const asksOther = $derived(command !== null && session.info.harness === 'codex' && !prompt.title.includes(command));
  // The title may repeat a long command; it is clamped only when the command below shows it in full. Anything else it names
  // (every path a change or a grant covers, what a Codex ask is about) is never cut: a long title scrolls instead.
  const clampTitle = $derived(command !== null && !asksOther);
  const readingKey = $derived(READINGS[session.info.harness]);
  const reading = $derived(readingKey && subject?.intent?.trim() ? t(readingKey, { reading: subject.intent.trim() }) : null);
  // The scrolling title is a tab stop only while it has more to show.
  let titleOverflows = $state(false);

  function answer(option: string): void {
    void session.respond(prompt.id, { type: 'value', value: option });
  }

  /** Alt+Y approves, Alt+N and Escape deny. `code` is used because Option+letter types symbols on macOS. */
  function onkeydown(event: KeyboardEvent): void {
    if (event.defaultPrevented) {
      return;
    }

    if (event.altKey && !event.ctrlKey && !event.metaKey && event.code === 'KeyY') {
      event.preventDefault();
      answer(prompt.approve);
    } else if (event.altKey && !event.ctrlKey && !event.metaKey && event.code === 'KeyN') {
      event.preventDefault();
      answer(prompt.deny);
    } else if (event.key === 'Escape' && !document.querySelector('[role="dialog"][aria-modal="true"]')) {
      event.preventDefault();
      answer(prompt.deny);
    }
  }
</script>

<svelte:window {onkeydown} />

<section aria-label={t('chat.approvalCard.label')} aria-live="assertive" data-approval-card class="chat-col mb-2">
  <div class="chat-radius flex flex-col gap-3 border border-warning/40 bg-warning-wash px-4 py-3.5 shadow-float">
    <div class="flex items-start gap-3">
      <Icon size={18} class="mt-0.5 shrink-0 text-warning" aria-hidden="true" />

      <div class="flex min-w-0 flex-1 flex-col gap-1">
        {#if clampTitle}
          <h3 class="line-clamp-2 text-base font-semibold break-words whitespace-pre-wrap text-fg" title={prompt.title}>{prompt.title}</h3>
        {:else}
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
          <h3 class="max-h-48 overflow-y-auto text-base font-semibold break-words whitespace-pre-wrap text-fg" tabindex={titleOverflows ? 0 : undefined} {@attach overflowing(prompt.title, (overflows) => (titleOverflows = overflows))}>{prompt.title}</h3>
        {/if}
        {#if subject}
          <p class="text-sm text-fg-2">{reading ?? toolTitle(subject)}{#if argument && !command} · <span class="font-mono break-all">{argument}</span>{/if}</p>
        {/if}
        {#if prompt.approveDetail}<p class="text-sm text-fg-2">{prompt.approveDetail}</p>{/if}
      </div>

      {#if prompt.timeoutMs}<Countdown timeoutMs={prompt.timeoutMs} requestId={prompt.id} ontimeout={() => answer(prompt.deny)} />{/if}
    </div>

    {#if command}<TerminalBlock text="" {command} capCommand />{/if}

    <div class="flex flex-wrap items-center gap-2">
      <Button variant="primary" onclick={() => answer(prompt.approve)}>
        {prompt.approve}
        {#snippet trailing()}<Kbd keys="alt+y" />{/snippet}
      </Button>
      <Button variant="danger" onclick={() => answer(prompt.deny)}>
        {prompt.deny}
        {#snippet trailing()}<Kbd keys="alt+n" />{/snippet}
      </Button>
      {#each prompt.others as other (other.option)}
        <Button onclick={() => answer(other.option)}>{other.option}</Button>
      {/each}
      <span class="ml-auto text-xs text-fg-3"><Kbd keys="escape" /> {t('chat.approvalCard.denies')}</span>
    </div>
  </div>
</section>
