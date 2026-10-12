<script lang="ts">
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  import TerminalBlock from './TerminalBlock.svelte';

  interface Props {
    text: string | null;
    running: boolean;
    failed: boolean;
  }

  let { text, running, failed }: Props = $props();

  const PREVIEW_LINES = 24;
  const PREVIEW_CHARS = 3000;

  let expanded = $state(false);

  const lines = $derived((text ?? '').split('\n'));
  const long = $derived(lines.length > PREVIEW_LINES || (text ?? '').length > PREVIEW_CHARS);
  const shown = $derived.by(() => {
    const full = text ?? '';
    if (expanded || !long) {
      return full;
    }

    return lines.slice(0, PREVIEW_LINES).join('\n').slice(0, PREVIEW_CHARS);
  });
</script>

{#if text !== null && text !== ''}
  <div class="flex flex-col gap-1.5">
    <TerminalBlock text={shown} {running} class={cx(failed && 'border-danger-edge')} />

    {#if long}
      <button type="button" class="self-start rounded-sm text-xs text-accent-text underline underline-offset-2 transition-colors hover:text-fg" onclick={() => (expanded = !expanded)}>
        {expanded ? t('chat.tool.showLess') : tn('chat.tool.showAll', lines.length)}
      </button>
    {/if}
  </div>
{:else if running}
  <p class="text-xs text-fg-3">{t('chat.tool.waiting')}</p>
{/if}
