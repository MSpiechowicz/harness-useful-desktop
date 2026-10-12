<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';

  import { i18n, LANGS, t, toLang, type Lang } from '$lib/i18n/i18n.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { cx } from '$lib/utils/cx';

  import { ICON } from './icon';
  import Popover from './Popover.svelte';

  interface Props {
    /** Fills a form column (Settings) instead of sitting in a toolbar. */
    wide?: boolean;
    /** Opens the list above the control, for the sidebar footer at the bottom of the window. */
    up?: boolean;
  }

  let { wide = false, up = false }: Props = $props();

  let open = $state(false);

  const shown = $derived(LANGS.find((lang) => lang.code === i18n.lang)?.label ?? '');
  const triggerLabel = $derived(t('common.languageCurrent', { language: shown }));

  function choose(code: Lang, close: () => void): void {
    close();

    // The saved language, not the one on screen: the screen lags it while a locale chunk loads.
    if (code !== toLang(prefs.value?.language)) {
      void prefs.update({ language: code });
    }
  }
</script>

<!-- Every language is named in itself. Saving the choice (not calling the i18n store) switches the UI: the language
     store follows the saved preference, so a failed save rolls the language back too. -->
<Popover bind:open label={t('common.language')} kind="menu" side={up ? 'top' : 'bottom'} class="w-max min-w-40" rootClass={wide ? 'w-full' : undefined}>
  {#snippet trigger({ props })}
    <button
      type="button"
      {...props}
      aria-label={triggerLabel}
      title={triggerLabel}
      class={cx('footer-control focus-visible:outline-2 focus-visible:outline-offset-1', wide && 'w-full')}
    >
      <!-- Every language stacked in one cell, only the current one visible: the control keeps the widest name's
           width, so picking another never shifts the controls beside it. -->
      <span class="inline-grid min-w-0 align-bottom">
        {#each LANGS as lang (lang.code)}
          <span class={cx('truncate text-left [grid-area:1/1]', lang.code !== i18n.lang && 'invisible')} aria-hidden={lang.code !== i18n.lang}>{lang.label}</span>
        {/each}
      </span>
      <ChevronDown size={14} class={cx('shrink-0 text-fg-3 transition-transform duration-[var(--dur-fast)]', open && 'rotate-180')} aria-hidden="true" />
    </button>
  {/snippet}

  {#snippet children({ close })}
    <div class="flex flex-col gap-0.5">
      {#each LANGS as lang (lang.code)}
        {@const selected = lang.code === i18n.lang}
        <button
          type="button"
          role="menuitemradio"
          aria-checked={selected}
          data-menu-item
          lang={lang.code}
          onclick={() => choose(lang.code, close)}
          class="flex h-9 w-full items-center gap-2.5 rounded-md px-2 text-left text-sm text-fg transition-colors duration-[var(--dur-fast)] hover:bg-hover active:bg-press"
        >
          <span class="min-w-0 flex-1 truncate">{lang.label}</span>
          {#if selected}<Check size={ICON.sm} class="shrink-0 text-accent-text" aria-hidden="true" />{/if}
        </button>
      {/each}
    </div>
  {/snippet}
</Popover>
