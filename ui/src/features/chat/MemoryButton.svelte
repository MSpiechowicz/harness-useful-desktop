<script lang="ts">
  import { untrack } from 'svelte';
  import Brain from '@lucide/svelte/icons/brain';
  import Pause from '@lucide/svelte/icons/pause';

  import Popover from '$lib/components/Popover.svelte';
  import { ICON } from '$lib/components/icon';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { everythingOff, GUARD_KINDS, offReason } from '$lib/stores/memory-chat';
  import { memoryChat } from '$lib/stores/memoryChat.svelte';
  import { memoryHeld } from '$lib/stores/memoryHeld.svelte';
  import { cx } from '$lib/utils/cx';

  import { repositoryOf } from './memory-repository';
  import MemoryPopover from './MemoryPopover.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  let open = $state(false);

  const memory = $derived(memoryChat.get(session.key));
  // The chat's own guards apply only when its harness passed the chat's key on to Harness Memory (a Codex chat may have no server to pass it to).
  // A Codex chat learns it again for every new thread and says so in its state, which then counts over what it said at the start.
  const passedOn = $derived(session.chat.session?.memoryGuards ?? session.info.memoryGuards);
  const capable = $derived(harness.get(session.info.harness)?.capabilities.memoryChatGuards === true && passedOn);
  // Nothing about saving can be said while it is not known that Harness Memory applies the opt-in guards (an older one refuses them), so the button is not shown.
  const shown = $derived(capable && memoryChat.supported && memory !== null);

  const repository = $derived(repositoryOf(session.info.project));
  const root = $derived(repository?.root ?? null);

  const waiting = $derived(memory ? memoryHeld.of(memory.key).length : 0);
  const paused = $derived(memory?.guards.paused ?? false);
  const allOff = $derived(memory !== null && memoryChat.global !== null && everythingOff(memory.guards, memoryChat.guards, root));
  // The server refuses saves while the guards cannot be read, and chats stay off while the old file is in use, whatever the switches say.
  const blocked = $derived(memoryChat.globalFailed || memoryChat.global?.migrationPending === true);
  const pausedEverywhere = $derived(allOff && memory !== null && GUARD_KINDS.every((kind) => offReason(kind, memory.guards, memoryChat.guards, root) === 'global'));
  // Three states, one chip: red when nothing can be saved, amber when the user paused saving, grey when anything can be saved.
  const look = $derived.by(() => {
    if (blocked) {
      return 'disabled';
    }

    if (paused || pausedEverywhere) {
      return 'paused';
    }

    return allOff ? 'disabled' : 'enabled';
  });
  const label = $derived(look === 'enabled' ? t('memory.guards.button') : t(look === 'paused' ? 'memory.guards.button.paused' : 'memory.guards.button.off'));

  // What every chat allows is read when the chat shows, so the button is right before it is opened, and read again by itself while it could not be.
  // Only `shown` may start it: the read changes the state it reads, so tracking it would re-run this effect after every read.
  $effect(() => {
    if (shown) {
      return untrack(() => memoryChat.watchGlobal());
    }
  });

  // Opening the popover reads again, so it is right when looked at, and asks again for the project's root, which a folder that came back now has.
  $effect(() => {
    if (open) {
      untrack(() => {
        void memoryChat.loadGlobal();
        void memoryChat.rootOf(session.info.project);
      });
    }
  });
</script>

<!-- Nothing is said until what every chat allows is known (read, or could not be read): the default is off, so a guess would show the wrong chip first. -->
{#if shown && memory && (memoryChat.global !== null || memoryChat.globalFailed)}
  <Popover label={t('memory.guards.popover')} kind="dialog" align="end" bind:open class="w-[26rem] p-0!">
    {#snippet trigger({ props })}
      <button
        type="button"
        {...props}
        aria-label={waiting > 0 ? `${label}, ${t('memory.guards.waitingAria', { count: waiting })}` : label}
        class={cx(
          'inline-flex h-control items-center gap-1.5 rounded-md px-2.5 text-sm font-medium tracking-wide whitespace-nowrap transition-[color,background-color,filter] duration-[var(--dur-fast)]',
          look === 'disabled' && 'bg-danger-wash text-danger hover:brightness-125 aria-expanded:brightness-125',
          look === 'paused' && 'bg-warning-wash text-warning hover:brightness-125 aria-expanded:brightness-125',
          look === 'enabled' && 'bg-surface-3 text-fg hover:brightness-125 aria-expanded:brightness-125',
        )}
      >
        {#if look === 'paused'}<Pause size={ICON.sm} aria-hidden="true" class="shrink-0" />{:else}<Brain size={ICON.sm} aria-hidden="true" class="shrink-0" />{/if}
        {label}
        {#if waiting > 0}
          <span class={cx('tabular rounded-full px-1.5 text-xs leading-5 font-medium text-warning', look === 'paused' ? 'bg-warning/25' : 'bg-warning-wash')}>{waiting}</span>
        {/if}
      </button>
    {/snippet}

    {#snippet children({ close })}
      <MemoryPopover agentKey={session.key} {root} folder={session.info.project} repoName={repository?.name ?? ''} {waiting} onclose={close} />
    {/snippet}
  </Popover>
{/if}
