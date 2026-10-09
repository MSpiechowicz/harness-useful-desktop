<script lang="ts">
  import FolderSearch from '@lucide/svelte/icons/folder-search';
  import type { Snippet } from 'svelte';

  import Badge from '$lib/components/Badge.svelte';
  import IconButton from '$lib/components/IconButton.svelte';
  import { revealPath } from '$lib/ipc/actions';

  import type { Fact } from './extension-detail';
  import type { ExtBadge } from './extension-items';

  interface Props {
    title: string;
    badges: ExtBadge[];
    description: string | null;
    facts: Fact[];
    /** Tab-specific sections below the facts. */
    children?: Snippet;
  }

  let { title, badges, description, facts, children }: Props = $props();
</script>

<article class="flex min-h-full flex-col gap-5 p-6" aria-labelledby="extension-title">
  <header class="flex flex-col gap-2">
    <h2 id="extension-title" class="text-2xl break-words text-fg">{title}</h2>

    <div class="flex flex-wrap gap-1.5">
      {#each badges as badge (badge.label)}
        <Badge tone={badge.tone}>{badge.label}</Badge>
      {/each}
    </div>

    {#if description}
      <p class="text-base text-fg-2">{description}</p>
    {/if}
  </header>

  {#if facts.length > 0}
    <dl class="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-2.5 rounded-lg border border-line bg-surface-1 p-4 text-sm">
      {#each facts as fact (fact.label)}
        <dt class="text-fg-3">{fact.label}</dt>
        <dd class="flex min-w-0 items-center gap-2 text-fg {fact.mono ? 'font-mono' : ''}">
          <span class="min-w-0 break-all">{fact.value}</span>
          {#if fact.path}
            <IconButton icon={FolderSearch} label="Reveal {fact.label.toLowerCase()} in file manager" size="sm" onclick={() => void revealPath(fact.value)} />
          {/if}
        </dd>
      {/each}
    </dl>
  {/if}

  {@render children?.()}
</article>
