<script lang="ts">
  import Monitor from '@lucide/svelte/icons/monitor';
  import Moon from '@lucide/svelte/icons/moon';
  import Sun from '@lucide/svelte/icons/sun';

  import { t } from '$lib/i18n/i18n.svelte';
  import type { ThemePref } from '$lib/ipc/bindings/ThemePref';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { rovingIndex } from '$lib/utils/roving';

  import { ICON } from './icon';
  import SegmentedControl from './SegmentedControl.svelte';
  import type { StripOption } from './strip';

  interface Props {
    /** Names each choice beside its icon in the desktop pill (a settings row); otherwise the footer's icon squares. */
    wide?: boolean;
  }

  let { wide = false }: Props = $props();

  const options = $derived<StripOption<ThemePref>[]>([
    { value: 'system', label: t('common.theme.system'), icon: Monitor },
    { value: 'light', label: t('common.theme.light'), icon: Sun },
    { value: 'dark', label: t('common.theme.dark'), icon: Moon },
  ]);
  const current = $derived(prefs.value?.theme ?? 'system');

  let buttons: (HTMLButtonElement | undefined)[] = $state([]);

  function choose(theme: ThemePref): void {
    if (theme !== current) {
      void prefs.update({ theme });
    }
  }

  function onKeydown(event: KeyboardEvent, index: number): void {
    const next = rovingIndex(event.key, index, options.length, 'horizontal');
    if (next === null) {
      return;
    }

    event.preventDefault();
    choose(options[next].value);
    buttons[next]?.focus();
  }
</script>

{#if wide}
  <SegmentedControl {options} value={current} label={t('common.theme')} onchange={choose} />
{:else}
  <!-- The dashboard's icon choices: 32px squares, the hairline an outline drawn inside them. -->
  <div class="seg-icons" role="radiogroup" aria-label={t('common.theme')}>
    {#each options as option, index (option.value)}
      {@const Icon = option.icon}
      <button
        bind:this={buttons[index]}
        type="button"
        role="radio"
        aria-checked={option.value === current}
        aria-label={option.label}
        title={option.label}
        tabindex={option.value === current ? 0 : -1}
        onclick={() => choose(option.value)}
        onkeydown={(event) => onKeydown(event, index)}
      >
        {#if Icon}<Icon size={ICON.sm - 2} strokeWidth={1.8} aria-hidden="true" />{/if}
      </button>
    {/each}
  </div>
{/if}
