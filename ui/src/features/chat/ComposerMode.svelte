<script lang="ts">
  import type { BusyMode } from '$lib/chat/composer';
  import { cx } from '$lib/utils/cx';
  import { t } from '$lib/i18n/i18n.svelte';

  interface Props {
    value: BusyMode;
  }

  let { value = $bindable() }: Props = $props();

  const OPTIONS: { mode: BusyMode; label: 'chat.composer.steer' | 'chat.composer.mode.queue'; hint: 'chat.composer.mode.steerHint' | 'chat.composer.mode.queueHint' }[] = [
    { mode: 'steer', label: 'chat.composer.steer', hint: 'chat.composer.mode.steerHint' },
    { mode: 'followUp', label: 'chat.composer.mode.queue', hint: 'chat.composer.mode.queueHint' },
  ];
</script>

<!-- What a message does while the agent works. Steer is the default: it is picked up at the agent's next step and
     nothing is interrupted; Queue waits until the agent has finished. -->
<div role="radiogroup" aria-label={t('chat.composer.mode.label')} class="inline-flex h-control items-center gap-0.5 rounded-md border border-line bg-surface-2 p-[3px]">
  {#each OPTIONS as option (option.mode)}
    {@const on = value === option.mode}
    <button
      type="button"
      role="radio"
      aria-checked={on}
      title={t(option.hint)}
      onclick={() => (value = option.mode)}
      class={cx(
        'inline-flex h-full items-center rounded-[9px] px-3 text-sm transition-colors duration-[var(--dur-fast)]',
        on ? (option.mode === 'steer' ? 'bg-accent-wash text-accent-text' : 'bg-surface-4 text-fg') : 'text-fg-3 hover:text-fg',
      )}
    >
      {t(option.label)}
    </button>
  {/each}
</div>
