<script lang="ts">
  import { untrack } from 'svelte';
  import Brain from '@lucide/svelte/icons/brain';
  import Pause from '@lucide/svelte/icons/pause';

  import Popover from '$lib/components/Popover.svelte';
  import { ICON } from '$lib/components/icon';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { everythingOff } from '$lib/stores/memory-chat';
  import { memoryChat } from '$lib/stores/memoryChat.svelte';
  import { memoryHeld } from '$lib/stores/memoryHeld.svelte';
  import { memoryRemote } from '$lib/stores/memoryRemote.svelte';
  import { revalidateProjectScopes } from '$lib/stores/project-scopes.svelte';
  import { cx } from '$lib/utils/cx';

  import { repositoryOf } from './memory-repository';
  import MemoryPopover from './MemoryPopover.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  let open = $state(false);

  const memory = $derived(memoryChat.get(session.key));
  const capable = $derived(harness.get(session.info.harness)?.capabilities.memoryChatGuards === true);
  // Nothing about saving can be said while it is not known that Harness Memory applies guards, so the button is not shown.
  const shown = $derived(capable && memoryRemote.value?.saveGuardsSupported === true && memory !== null);

  const repository = $derived(repositoryOf(session.info.project));
  const root = $derived(repository?.root ?? null);

  const waiting = $derived(memory ? memoryHeld.of(memory.key).length : 0);
  const paused = $derived(memory?.guards.paused ?? false);
  const allOff = $derived(memory ? everythingOff(memory.guards, memoryChat.guards, root) : false);
  const label = $derived.by(() => {
    if (paused) {
      return t('memory.guards.button.paused');
    }

    return allOff ? t('memory.guards.button.off') : t('memory.guards.button');
  });

  // Which repository the chat is in and what every chat allows are read when the chat shows, so the button is right before it is opened.
  // Only `shown` may start it: the reads change the state they read (the project list's loader), so tracking them re-ran this
  // effect after every read and kept the app redrawing.
  $effect(() => {
    if (shown) {
      untrack(() => {
        void memoryChat.loadGlobal();
        revalidateProjectScopes();
      });
    }
  });
</script>

{#if shown && memory}
  <Popover label={t('memory.guards.popover')} kind="dialog" align="end" bind:open class="w-[26rem] p-0!">
    {#snippet trigger({ props })}
      <button
        type="button"
        {...props}
        aria-label={waiting > 0 ? `${label}, ${t('memory.guards.waitingAria', { count: waiting })}` : label}
        class={cx(
          'inline-flex h-control items-center gap-1.5 rounded-md px-2 text-sm transition-colors duration-[var(--dur-fast)]',
          paused || allOff ? 'bg-warning-wash text-warning' : 'text-fg-2 hover:bg-hover hover:text-fg aria-expanded:bg-hover aria-expanded:text-fg',
        )}
      >
        {#if paused}<Pause size={ICON.sm} aria-hidden="true" class="shrink-0" />{:else}<Brain size={ICON.sm} aria-hidden="true" class="shrink-0" />{/if}
        {label}
        {#if waiting > 0}
          <span class="tabular rounded-full bg-warning-wash px-1.5 text-xs leading-5 font-medium text-warning">{waiting}</span>
        {/if}
      </button>
    {/snippet}

    {#snippet children({ close })}
      <MemoryPopover agentKey={session.key} {root} repoName={repository?.name ?? ''} {waiting} onclose={close} />
    {/snippet}
  </Popover>
{/if}
