<script lang="ts">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';

  import type { ChatSession } from '$lib/chat/session.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';
  import { harness } from '$lib/stores/harness.svelte';
  import { cx } from '$lib/utils/cx';

  import { switchHarness } from '../shell/actions';
  import MenuItem from './MenuItem.svelte';

  interface Props {
    session: ChatSession;
  }

  let { session }: Props = $props();

  /** The dot takes the harness accent (`currentColor`), like its badge elsewhere. */
  const DOT: Record<string, string> = {
    omp: 'text-omp',
    pi: 'text-pi',
    opencode: 'text-opencode',
    claude: 'text-claude',
  };
  /** A harness without accent tokens of its own gets a neutral dot. */
  const NEUTRAL = 'text-fg-3';

  const current = $derived(harness.get(session.info.harness));
  // While the chat moves to another harness, the button already shows it, a spinner in the chevron's place.
  const shown = $derived((session.switchingTo ? harness.get(session.switchingTo) : null) ?? current);

  /** What a row says beside its name: why it cannot be chosen, else its version. */
  function detailOf(item: HarnessSummary): string | null {
    if (!item.detection.installed) {
      return t('chat.harness.notInstalled');
    }

    if (!item.capabilities.chat) {
      return t('chat.harness.comingSoon');
    }

    const version = item.detection.version ? `v${item.detection.version}` : null;

    // A preview says so in plain words beside the version: rows of a menu carry no badges.
    return item.capabilities.chatPreview ? [t('chat.preview.label'), version].filter(Boolean).join(' · ') : version;
  }

  /** The row's tooltip: what a preview means, for a harness whose chat is one and can be chosen. */
  function hintOf(item: HarnessSummary): string | null {
    const chooseable = item.detection.installed && item.capabilities.chat;

    return chooseable && item.capabilities.chatPreview ? t('chat.preview.tooltip', { harness: item.info.name }) : null;
  }
</script>

{#if current && shown}
  <Popover label={t('chat.harness.label')} kind="menu" side="top" class="w-max min-w-52 max-w-md!">
    {#snippet trigger({ props })}
      <!-- A live dot in the harness's colour and its full name. A narrow composer keeps only the dot. -->
      <button
        type="button"
        {...props}
        aria-label={t('chat.harness.aria', { name: shown.info.name })}
        class="group inline-flex h-control min-w-0 items-center gap-2 rounded-md px-2 text-sm text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg aria-expanded:bg-hover aria-expanded:text-fg"
      >
        <span aria-hidden="true" class={cx('live-dot shrink-0', DOT[shown.info.id] ?? NEUTRAL)}></span>
        <span class="whitespace-nowrap [text-box:trim-both_cap_alphabetic] @max-[460px]:hidden">{shown.info.name}</span>
        {#if session.switchingTo}
          <span class="grid size-4 shrink-0 place-items-center @max-[460px]:hidden"><Spinner size={14} class="text-accent-text" label={t('chat.tabs.starting')} /></span>
        {:else}
          <ChevronDown
            size={16}
            aria-hidden="true"
            class="shrink-0 text-fg-3 transition-transform duration-[var(--dur-base)] ease-[var(--ease-out-expo)] group-aria-expanded:rotate-180 @max-[460px]:hidden"
          />
        {/if}
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
            hint={hintOf(item)}
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
