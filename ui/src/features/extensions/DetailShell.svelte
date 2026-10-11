<script lang="ts">
  import FolderSearch from '@lucide/svelte/icons/folder-search';
  import type { Snippet } from 'svelte';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
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

  // A fact the badges already state (scope, version, marketplace) is not said a second time.
  const shown = $derived(facts.filter((fact) => !badges.some((badge) => badge.label === fact.value || badge.label === `v${fact.value}`)));
</script>

<!-- One column: title and badges, the remaining facts as plain rows, then whatever the tab adds. Nothing floats. -->
<article class="flex min-h-full max-w-5xl flex-col gap-5 p-6" aria-labelledby="extension-title">
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

  {#if shown.length > 0}
    <dl class="divide-y divide-line rounded-lg border border-line bg-surface-1 text-sm">
      {#each shown as fact (fact.label)}
        <div class="flex min-h-10 items-center gap-4 py-1.5 pr-1.5 pl-3">
          <dt class="w-32 shrink-0 text-fg-3">{fact.label}</dt>
          <dd class={['min-w-0 flex-1 truncate text-fg', fact.mono && 'font-mono']} title={fact.value}>{fact.value}</dd>
          {#if fact.path}
            <Button size="sm" variant="ghost" onclick={() => void revealPath(fact.value)}>
              {#snippet leading()}<FolderSearch size={ICON.sm} aria-hidden="true" />{/snippet}
              Reveal
            </Button>
          {/if}
        </div>
      {/each}
    </dl>
  {/if}

  {@render children?.()}
</article>
