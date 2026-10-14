<script lang="ts">
  import ShieldAlert from '@lucide/svelte/icons/shield-alert';

  import Button from '$lib/components/Button.svelte';
  import Kbd from '$lib/components/Kbd.svelte';
  import { approvalSubject, type ApprovalPrompt } from '$lib/chat/approval';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { commandOf, primaryArg, TOOL_ICONS, toolKind, toolTitle } from '$lib/chat/tool-meta';
  import { t } from '$lib/i18n/i18n.svelte';

  import Countdown from './Countdown.svelte';
  import TerminalBlock from './TerminalBlock.svelte';

  interface Props {
    session: ChatSession;
    prompt: ApprovalPrompt;
  }

  let { session, prompt }: Props = $props();

  const subject = $derived(approvalSubject(session.chat));
  const Icon = $derived(subject ? TOOL_ICONS[toolKind(subject.name)] : ShieldAlert);
  const command = $derived(subject ? commandOf(subject) : null);
  const argument = $derived(subject ? primaryArg(subject.args) : null);

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
        <h3 class="text-base font-semibold text-fg">{prompt.title}</h3>
        {#if subject}
          <p class="text-sm text-fg-2">{toolTitle(subject)}{#if argument && !command} · <span class="font-mono">{argument}</span>{/if}</p>
        {/if}
        {#if prompt.approveDetail}<p class="text-sm text-fg-2">{prompt.approveDetail}</p>{/if}
      </div>

      {#if prompt.timeoutMs}<Countdown timeoutMs={prompt.timeoutMs} requestId={prompt.id} ontimeout={() => answer(prompt.deny)} />{/if}
    </div>

    {#if command}<TerminalBlock text="" {command} />{/if}

    <div class="flex flex-wrap items-center gap-2">
      <Button variant="primary" size="sm" onclick={() => answer(prompt.approve)}>
        {prompt.approve}
        {#snippet trailing()}<Kbd keys="alt+y" />{/snippet}
      </Button>
      <Button variant="danger" size="sm" onclick={() => answer(prompt.deny)}>
        {prompt.deny}
        {#snippet trailing()}<Kbd keys="alt+n" />{/snippet}
      </Button>
      {#each prompt.others as other (other.option)}
        <Button size="sm" onclick={() => answer(other.option)}>{other.option}</Button>
      {/each}
      <span class="ml-auto text-xs text-fg-3"><Kbd keys="escape" /> {t('chat.approvalCard.denies')}</span>
    </div>
  </div>
</section>
