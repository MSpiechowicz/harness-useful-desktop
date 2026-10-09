<script lang="ts">
  import FolderSearch from '@lucide/svelte/icons/folder-search';
  import type { Snippet } from 'svelte';

  import Badge from '$lib/components/Badge.svelte';
  import IconButton from '$lib/components/IconButton.svelte';
  import { revealPath } from '$lib/ipc/actions';
  import { breakable } from '$lib/utils/format';

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
      <p class="max-w-3xl text-base text-fg-2">{description}</p>
    {/if}
  </header>

  <!-- Wide panes put the facts beside the tab-specific sections instead of leaving that space empty. -->
  <div class="grid gap-5 @4xl:grid-cols-[minmax(0,1fr)_22rem] @4xl:items-start">
    {#if facts.length > 0}
      <dl class="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-2.5 rounded-lg border border-line bg-surface-1 p-4 text-sm @4xl:sticky @4xl:top-0 @4xl:order-last">
        {#each facts as fact (fact.label)}
          <dt class="text-fg-3">{fact.label}</dt>
          <dd class="flex min-w-0 items-center gap-2 text-fg {fact.mono ? 'font-mono' : ''}">
            <span class="min-w-0 [overflow-wrap:anywhere]">{fact.path ? breakable(fact.value) : fact.value}</span>
            {#if fact.path}
              <IconButton icon={FolderSearch} label="Reveal {fact.label.toLowerCase()} in file manager" size="sm" onclick={() => void revealPath(fact.value)} />
            {/if}
          </dd>
        {/each}
      </dl>
    {/if}

    <div class="flex min-w-0 flex-col gap-5">
      {@render children?.()}
    </div>
  </div>
</article>
