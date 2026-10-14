<script lang="ts">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';

  import Card from '$lib/components/Card.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { cx } from '$lib/utils/cx';
  import { formatCount } from '$lib/utils/format';

  interface Props {
    names: string[];
    /** The backend's sentence on how to use a profile; the card shows it in the language on screen. */
    hint: string | null;
  }

  let { names, hint }: Props = $props();

  /** Rows shown before the rest fold behind "Show all". */
  const VISIBLE = 4;
  const FLAG = '--profile <name>';

  let all = $state(false);

  const shown = $derived(all ? names : names.slice(0, VISIBLE));
  const foldable = $derived(names.length > VISIBLE);
  const sentence = $derived(t('config.profiles.hint', { harness: harness.active?.info.shortName ?? '' }).split(FLAG));
</script>

<!-- One line per profile, like the rows of the other cards; the way to use one sits under a hairline. -->
<Card as="section" padded={false} class="p-3.5">
  <h3 class="mb-2 flex items-center justify-between gap-1.5 text-xs font-medium tracking-wider text-fg-3 uppercase">
    {t('config.profiles.title')}
    <span class="tabular tracking-normal">{formatCount(names.length)}</span>
  </h3>

  <ul class="flex flex-col">
    {#each shown as name (name)}
      <li class="flex h-7 items-center gap-2.5 text-sm text-fg-2" title={name}>
        <span class="size-1.5 shrink-0 rounded-full bg-fg-3" aria-hidden="true"></span>
        <span class="min-w-0 truncate font-mono text-xs">{name}</span>
      </li>
    {/each}
  </ul>

  {#if foldable}
    <button
      type="button"
      aria-expanded={all}
      class="mt-1 -ml-1.5 inline-flex h-7 items-center gap-1.5 rounded-sm px-1.5 text-xs text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg"
      onclick={() => (all = !all)}
    >
      <ChevronDown size={14} aria-hidden="true" class={cx('transition-transform duration-[var(--dur-fast)]', all && 'rotate-180')} />
      {all ? t('config.profiles.less') : t('config.profiles.all', { count: formatCount(names.length) })}
    </button>
  {/if}

  {#if hint}
    <p class="mt-2.5 border-t border-line pt-2.5 text-xs leading-normal text-fg-3">
      {sentence[0]}{#if sentence.length > 1}<code class="rounded-sm bg-surface-3 px-1 py-px font-mono text-[11.5px] text-fg-2">{FLAG}</code>{sentence[1]}{/if}
    </p>
  {/if}
</Card>
