<script lang="ts">
  import Webhook from '@lucide/svelte/icons/webhook';

  import type { HookText } from '$lib/chat/hook-context';
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

  const names = $derived(hooks.map((hook) => hook.name).join(' · '));
  const heading = $derived(kind === 'context' ? t('hooks.chip.added') : t('hooks.chip.sent', { names }));
</script>

<!-- What the hooks added to this message, or sent in its place, in the same row shape as the agent's tool calls. -->
<ChatRow {open} ontoggle={() => (open = !open)}>
  {#snippet icon()}<Webhook size={16} aria-hidden="true" />{/snippet}

  {#snippet title()}
    <span class="font-medium text-fg">{heading}</span>
    {#if kind === 'context'}<span class="ml-2 text-xs text-fg-3">{names}</span>{/if}
  {/snippet}

  {#snippet status()}
    <StatusPill tone="info" label={tn('hooks.chip.count', hooks.length)}>
      {#snippet icon()}<Webhook size={16} aria-hidden="true" />{/snippet}
    </StatusPill>
  {/snippet}

  {#snippet body()}
    <div class="flex flex-col gap-3 p-4">
      <p class="text-xs text-fg-3">{t(kind === 'context' ? 'hooks.chip.caption.context' : 'hooks.chip.caption.followUp')}</p>

      {#each hooks as hook, index (index)}
        <div class="flex flex-col gap-1.5">
          {#if hooks.length > 1 || kind === 'context'}<p class="text-xs font-medium text-fg-2">{hook.name}</p>{/if}
          <!-- Same panel as a tool's output (TerminalBlock), as a labelled region. -->
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
          <pre role="region" tabindex="0" aria-label={t('hooks.chip.text', { name: hook.name })} class="chat-radius max-h-72 overflow-auto border border-line bg-canvas px-3.5 py-3 font-mono text-[13px] leading-relaxed break-words whitespace-pre-wrap text-fg-2">{hook.text}</pre>
        </div>
      {/each}
    </div>
  {/snippet}
</ChatRow>
