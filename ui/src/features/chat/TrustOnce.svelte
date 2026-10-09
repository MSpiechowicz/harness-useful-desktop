<script lang="ts">
  import ShieldCheck from '@lucide/svelte/icons/shield-check';
  import ShieldQuestion from '@lucide/svelte/icons/shield-question-mark';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';

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
  <Badge tone="success"><ShieldCheck size={12} aria-hidden="true" /> Project trusted for this session</Badge>
{:else}
  <Popover label="Trust this project" kind="dialog" bind:open class="w-80">
    {#snippet trigger({ props })}
      <button type="button" {...props} class="inline-flex h-8 items-center gap-2 rounded-md border border-line bg-surface-2 px-2.5 text-sm text-fg-2 transition-colors hover:bg-surface-3 hover:text-fg">
        <ShieldQuestion size={14} aria-hidden="true" />
        Trust project once
      </button>
    {/snippet}

    {#snippet children()}
      <div class="flex flex-col gap-3 p-1.5">
        <p class="text-sm text-fg-2">
          PI skips this project's extensions, skills and prompt templates until you trust it. Trusting lets code from the project run with your permissions.
        </p>
        <p class="text-xs text-fg-3">It applies to this session only. The app asks again the next time PI starts. The session restarts and continues where it left off.</p>
        <Button variant="primary" size="sm" onclick={trust}>Restart and trust this project</Button>
      </div>
    {/snippet}
  </Popover>
{/if}
