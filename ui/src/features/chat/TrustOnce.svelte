<script lang="ts">
  import ShieldCheck from '@lucide/svelte/icons/shield-check';
  import ShieldQuestion from '@lucide/svelte/icons/shield-question-mark';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  let open = $state(false);

  async function trust(): Promise<void> {
    open = false;
    await agentSessions.restart(session.key, { trustOnce: true });
  }
</script>

{#if session.info.trustedOnce}
  <Badge tone="success" class="h-control rounded-md! px-2.5 text-sm!"><ShieldCheck size={16} aria-hidden="true" /> {t('chat.trust.done')}</Badge>
{:else}
  <Popover label={t('chat.trust.popover')} kind="dialog" bind:open class="w-80">
    {#snippet trigger({ props })}
      <button type="button" {...props} class="inline-flex h-control items-center gap-2 rounded-md border border-line bg-surface-2 px-2.5 text-sm text-fg-2 transition-colors hover:bg-surface-3 hover:text-fg">
        <ShieldQuestion size={16} aria-hidden="true" />
        {t('chat.trust.trigger')}
      </button>
    {/snippet}

    {#snippet children()}
      <div class="flex flex-col gap-3 p-1.5">
        <p class="text-sm text-fg-2">{t('chat.trust.warning')}</p>
        <p class="text-xs text-fg-3">{t('chat.trust.scope')}</p>
        <Button variant="primary" onclick={trust}>{t('chat.trust.action')}</Button>
      </div>
    {/snippet}
  </Popover>
{/if}
