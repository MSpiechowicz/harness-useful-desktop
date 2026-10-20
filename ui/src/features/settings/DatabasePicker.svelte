<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import Popover from '$lib/components/Popover.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MemoryRemoteKind } from '$lib/ipc/bindings/MemoryRemoteKind';
  import { cx } from '$lib/utils/cx';

  import { KIND_LABELS, REMOTE_KINDS } from './memory-remote';

  interface Props {
    value: MemoryRemoteKind;
    onchange: (kind: MemoryRemoteKind) => void;
  }

  let { value, onchange }: Props = $props();

  let open = $state(false);

  const label = $derived(t('settings.memoryRemote.database.title'));
  const current = $derived(t(KIND_LABELS[value]));
</script>

<!-- A short list of ready-made choices, so a menu rather than a free-text field. -->
<Popover bind:open {label} kind="menu" align="end" class="w-72 min-w-0!">
  {#snippet trigger({ props })}
    <!-- Focus shows as the field border, like the URL field next to it, not as a ring around the border. -->
    <Button {...props} bare aria-label={`${label}: ${current}`} class="w-72 justify-between! focus-visible:outline-none focus-visible:border-focus">
      <span class="min-w-0 truncate">{current}</span>
      {#snippet trailing()}<ChevronDown size={ICON.sm} class={cx('transition-transform duration-[var(--dur-fast)]', open && 'rotate-180')} aria-hidden="true" />{/snippet}
    </Button>
  {/snippet}

  {#snippet children({ close })}
    <div role="none" class="flex flex-col">
      {#each REMOTE_KINDS as kind (kind)}
        <button
          type="button"
          role="menuitemradio"
          aria-checked={kind === value}
          data-menu-item
          class={cx(
            'flex items-center justify-between gap-2 rounded-sm px-2.5 py-2 text-left text-sm transition-colors duration-[var(--dur-fast)] hover:bg-hover focus-visible:bg-hover',
            kind === value ? 'text-fg' : 'text-fg-2',
          )}
          onclick={() => {
            close();
            onchange(kind);
          }}
        >
          {t(KIND_LABELS[kind])}
          {#if kind === value}<Check size={ICON.sm} class="text-accent-text" aria-hidden="true" />{/if}
        </button>
      {/each}
    </div>
  {/snippet}
</Popover>
