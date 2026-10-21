<script lang="ts">
  import Webhook from '@lucide/svelte/icons/webhook';

  import { type HookText, MEMORY_SETTINGS_PART } from '$lib/chat/hook-context';
  import { t, tn } from '$lib/i18n/i18n.svelte';

  import ChatRow from './ChatRow.svelte';
  import StatusPill from './StatusPill.svelte';

  interface Props {
    /** The hooks whose text this is. */
    hooks: HookText[];
    /** Context hooks put in front of the user's message, or the message a hook sent to the agent. */
    kind: 'context' | 'followUp';
  }

  let { hooks, kind }: Props = $props();

  let open = $state(false);

  // The app's own part (what the chat's Memory settings allow, told to the agent) is not a hook: it is named and counted apart.
  const isMemory = (hook: HookText): boolean => kind === 'context' && hook.name === MEMORY_SETTINGS_PART;
  const nameOf = (hook: HookText): string => (isMemory(hook) ? t('hooks.chip.memory.name') : hook.name);
  const real = $derived(hooks.filter((hook) => !isMemory(hook)));
  const hasMemory = $derived(real.length < hooks.length);

  const names = $derived(hooks.map(nameOf).join(' · '));
  const heading = $derived.by(() => {
    if (kind !== 'context') {
      return t('hooks.chip.sent', { names });
    }

    return real.length === 0 ? t('hooks.chip.memory.added') : t('hooks.chip.added');
  });
  const caption = $derived.by(() => {
    if (kind !== 'context') {
      return t('hooks.chip.caption.followUp');
    }

    if (!hasMemory) {
      return t('hooks.chip.caption.context');
    }

    return real.length === 0 ? t('hooks.chip.caption.memory') : t('hooks.chip.caption.contextAndMemory');
  });
  const pill = $derived(real.length === 0 && hasMemory ? t('hooks.chip.memory.pill') : tn('hooks.chip.count', real.length));
</script>

<!-- What the hooks added to this message, or sent in its place, in the same row shape as the agent's tool calls. -->
<ChatRow {open} ontoggle={() => (open = !open)}>
  {#snippet icon()}<Webhook size={16} aria-hidden="true" />{/snippet}

  {#snippet title()}
    <span class="font-medium text-fg">{heading}</span>
    {#if kind === 'context'}<span class="ml-2 text-xs text-fg-3">{names}</span>{/if}
  {/snippet}

  {#snippet status()}
    <StatusPill tone="info" label={pill}>
      {#snippet icon()}<Webhook size={16} aria-hidden="true" />{/snippet}
    </StatusPill>
  {/snippet}

  {#snippet body()}
    <div class="flex flex-col gap-3 p-4">
      <p class="text-xs text-fg-3">{caption}</p>

      {#each hooks as hook, index (index)}
        <div class="flex flex-col gap-1.5">
          {#if hooks.length > 1 || kind === 'context'}<p class="text-xs font-medium text-fg-2">{nameOf(hook)}</p>{/if}
          <!-- Same panel as a tool's output (TerminalBlock), as a labelled region. -->
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
          <pre role="region" tabindex="0" aria-label={t('hooks.chip.text', { name: nameOf(hook) })} class="chat-radius max-h-72 overflow-auto border border-line bg-canvas px-3.5 py-3 font-mono text-[13px] leading-relaxed break-words whitespace-pre-wrap text-fg-2">{hook.text}</pre>
        </div>
      {/each}
    </div>
  {/snippet}
</ChatRow>
