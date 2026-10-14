<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { git } from '$lib/stores/git.svelte';

  interface Props {
    project: string;
    paths: string[];
  }

  let { project, paths }: Props = $props();

  const SHOWN = 8;
</script>

<div role="group" aria-label={t('sessions.touched.label')}>
  {#if paths.length === 0}
    <p class="text-sm text-fg-3">{t('sessions.touched.none')}</p>
  {:else}
    <ul class="flex flex-col">
      {#each paths.slice(0, SHOWN) as path (path)}
        <li>
          <button
            type="button"
            class="block w-full truncate rounded-sm px-1.5 py-1 text-left font-mono text-xs text-fg-2 transition-colors hover:bg-hover hover:text-fg active:bg-press"
            title={t('sessions.touched.show', { path })}
            onclick={() => git.showPaths(project, [path], t('sessions.touched.source'))}
          >
            {path}
          </button>
        </li>
      {/each}
    </ul>

    {#if paths.length > SHOWN}<p class="px-1.5 pt-1 text-xs text-fg-3">{t('sessions.touched.more', { count: paths.length - SHOWN })}</p>{/if}

    <Button variant="ghost" class="mt-2 -ml-2" onclick={() => git.showPaths(project, paths, t('sessions.touched.source'))}>{t('sessions.touched.view')}</Button>
  {/if}
</div>
