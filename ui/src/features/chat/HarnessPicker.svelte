<script lang="ts">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';

  import type { ChatSession } from '$lib/chat/session.svelte';
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';
  import { harness } from '$lib/stores/harness.svelte';

  import { switchHarness } from '../shell/actions';
  import MenuItem from './MenuItem.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  const current = $derived(harness.get(session.info.harness));

  /** What a row says beside its name: why it cannot be chosen, else its version. */
  function detailOf(item: HarnessSummary): string | null {
    if (!item.detection.installed) {
      return t('chat.harness.notInstalled');
    }

    if (!item.capabilities.chat) {
      return t('chat.harness.comingSoon');
    }

    return item.detection.version ? `v${item.detection.version}` : null;
  }
</script>

{#if current}
  <Popover label={t('chat.harness.label')} kind="menu" side="top" class="w-max min-w-52 max-w-md!">
    {#snippet trigger({ props })}
      <button
        type="button"
        {...props}
        aria-label={t('chat.harness.aria', { name: current.info.name })}
        class="inline-flex h-control min-w-0 items-center gap-1.5 rounded-md px-2 text-sm text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg aria-expanded:bg-hover aria-expanded:text-fg"
      >
        <HarnessBadge info={current.info} size="xs" />
        <span class="whitespace-nowrap [text-box:trim-both_cap_alphabetic] @max-[460px]:hidden">{current.info.shortName}</span>
        <ChevronDown size={16} aria-hidden="true" class="shrink-0 text-fg-3 @max-[460px]:hidden" />
      </button>
    {/snippet}

    {#snippet children({ close })}
      <!-- The same rows as the model picker beside it. Opens upward and scrolls: the list grows with every harness the app learns to talk to. -->
      <div class="flex max-h-72 flex-col overflow-y-auto" role="group" aria-label={t('chat.harness.label')}>
        {#each harness.summaries as item (item.info.id)}
          {@const installed = item.detection.installed}
          <!-- A row says why it cannot be chosen: the harness is not installed, or the app cannot chat with it yet. -->
          <MenuItem
            label={item.info.name}
            detail={detailOf(item)}
            selected={item.info.id === current.info.id}
            disabled={!installed || !item.capabilities.chat}
            onselect={() => {
              close();
              void switchHarness(item.info.id);
            }}
          />
        {/each}
      </div>
    {/snippet}
  </Popover>
{/if}
