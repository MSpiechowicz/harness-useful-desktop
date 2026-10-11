<script lang="ts">
  import { untrack } from 'svelte';

  import { autofocus } from '$lib/a11y/autofocus';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { agentSessions } from '$lib/stores/agentSession.svelte';

  interface Props {
    session: ChatSession;
    /** Called when editing ends, committed or cancelled. */
    ondone: () => void;
  }

  let { session, ondone }: Props = $props();

  // Starts from the name at the moment editing begins; later renames do not overwrite what is being typed.
  let value = $state(untrack(() => agentSessions.title(session.key)));

  function commit(): void {
    if (value.trim() !== '' && value.trim() !== agentSessions.title(session.key)) {
      void session.rename(value);
    }
    ondone();
  }

  function onkeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter') {
      event.preventDefault();
      commit();
    } else if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      ondone();
    }
  }
</script>

<!--
  The tab's name, edited in place (double-click or F2 on a tab): the text itself becomes editable, with no box of its
  own. An invisible copy of the text sizes the field, so it is always exactly as wide as the name.
-->
<span class="ml-1 inline-grid h-7 max-w-52 min-w-8 items-center text-sm">
  <span class="invisible col-start-1 row-start-1 overflow-hidden whitespace-pre" aria-hidden="true">{value || ' '}</span>
  <input
    bind:value
    use:autofocus
    {onkeydown}
    onblur={commit}
    onfocus={(event) => event.currentTarget.select()}
    aria-label="Chat name"
    maxlength={120}
    class="col-start-1 row-start-1 w-full min-w-0 bg-transparent p-0 text-fg outline-none"
  />
</span>
