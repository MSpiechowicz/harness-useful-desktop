<script lang="ts">
  import { onMount, tick } from 'svelte';

  import { t } from '$lib/i18n/i18n.svelte';
  import type { InstallLogLine } from '$lib/ipc/bindings/InstallLogLine';
  import { cx } from '$lib/utils/cx';

  interface Props {
    lines: InstallLogLine[];
  }

  let { lines }: Props = $props();

  let box = $state<HTMLElement>();

  // The step scrolls: a run that starts below the fold brings its output into view.
  onMount(() => box?.scrollIntoView({ block: 'nearest' }));

  // New output keeps the newest line in view.
  $effect(() => {
    void lines.length;

    void tick().then(() => box?.scrollTo({ top: box.scrollHeight }));
  });
</script>

<div
  bind:this={box}
  role="log"
  aria-label={t('onboarding.setup.log')}
  class="max-h-40 min-h-16 overflow-y-auto rounded-md border border-line bg-surface-2 px-3 py-2 font-mono text-xs leading-relaxed"
>
  {#each lines as line, index (index)}
    <p class={cx('[overflow-wrap:anywhere] whitespace-pre-wrap', line.stream === 'stderr' ? 'text-warning' : 'text-fg-2')}>{line.line || ' '}</p>
  {:else}
    <p class="text-fg-3">{t('onboarding.setup.log.waiting')}</p>
  {/each}
</div>
