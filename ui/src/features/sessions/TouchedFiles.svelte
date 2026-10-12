<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { git } from '$lib/stores/git.svelte';

  interface Props {
    project: string;
    paths: string[];
  }

  let { project, paths }: Props = $props();

  const SHOWN = 6;
</script>

{#if paths.length > 0}
  <div class="flex flex-wrap items-center gap-2" role="group" aria-label={t('sessions.touched.label')}>
    <span class="text-xs font-medium text-fg-3">{t('sessions.touched.label')}</span>

    {#each paths.slice(0, SHOWN) as path (path)}
      <button
        type="button"
        class="rounded-sm bg-surface-3 px-2 py-0.5 font-mono text-xs text-fg-2 transition-colors hover:bg-hover active:bg-press hover:text-fg"
        title={t('sessions.touched.show', { path })}
        onclick={() => git.showPaths(project, [path], t('sessions.touched.source'))}
      >
        {path}
      </button>
    {/each}

    {#if paths.length > SHOWN}<span class="text-xs text-fg-3">{t('sessions.touched.more', { count: paths.length - SHOWN })}</span>{/if}

    <Button size="sm" variant="ghost" onclick={() => git.showPaths(project, paths, t('sessions.touched.source'))}>{t('sessions.touched.view')}</Button>
  </div>
{/if}
