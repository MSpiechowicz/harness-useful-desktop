<script lang="ts">
  import { untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import Dialog from '$lib/components/Dialog.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { autofocus } from '$lib/a11y/autofocus';

  interface Props {
    session: ChatSession;
    kind: 'compact' | 'name';
  }

  let { session, kind }: Props = $props();

  const COPY = {
    compact: {
      title: 'Compact the conversation',
      description: 'Summarizes older messages to free up context. You can tell the agent what to keep.',
      label: 'Instructions (optional)',
      placeholder: 'Keep the decisions about the parser',
      submit: 'Compact',
    },
    name: {
      title: 'Name this session',
      description: 'The name shows in the tab and in your session list.',
      label: 'Session name',
      placeholder: 'Handle empty config input',
      submit: 'Save name',
    },
  } as const;

  const copy = $derived(COPY[kind]);

  let value = $state(untrack(() => (kind === 'name' ? (session.name ?? '') : '')));

  function close(): void {
    session.ui.ask = null;
  }

  async function submit(): Promise<void> {
    const text = value.trim();
    close();

    if (kind === 'compact') {
      await session.send({ type: 'compact', instructions: text === '' ? null : text });
    } else if (text !== '') {
      await session.rename(text);
    }
  }
</script>

<Dialog open title={copy.title} description={copy.description} onclose={close}>
  <label class="flex flex-col gap-1.5 text-sm text-fg-2">
    {copy.label}
    <input
      bind:value
      use:autofocus
      placeholder={copy.placeholder}
      onkeydown={(event) => event.key === 'Enter' && void submit()}
      class="h-10 w-full rounded-md border border-line-strong bg-surface-2 px-3 text-base text-fg outline-none focus:border-focus"
    />
  </label>

  {#snippet footer()}
    <Button onclick={close}>Cancel</Button>
    <Button variant="primary" onclick={submit}>{copy.submit}</Button>
  {/snippet}
</Dialog>
