<script lang="ts">
  import Pencil from '@lucide/svelte/icons/pencil';

  import IconButton from '$lib/components/IconButton.svelte';
  import { autofocus } from '$lib/a11y/autofocus';
  import type { ChatSession } from '$lib/chat/session.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  let editing = $state(false);
  let value = $state('');

  function start(): void {
    value = session.name ?? '';
    editing = true;
  }

  function commit(): void {
    editing = false;
    void session.rename(value);
  }

  function onkeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter') {
      event.preventDefault();
      commit();
    } else if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      editing = false;
    }
  }
</script>

{#if editing}
  <input
    bind:value
    use:autofocus
    {onkeydown}
    onblur={commit}
    aria-label="Session name"
    maxlength={120}
    class="h-8 min-w-0 flex-1 rounded-md border border-focus bg-surface-2 px-2.5 text-lg text-fg outline-none"
  />
{:else}
  <h2 class="min-w-0 truncate text-lg text-fg">{session.name ?? 'New chat'}</h2>
  <IconButton icon={Pencil} label="Rename session" size="sm" side="bottom" onclick={start} disabled={session.ended} />
{/if}
