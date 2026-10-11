<script lang="ts">
  import { formatTokens } from '$lib/dashboard/format';
  import type { TokenBreakdown } from '$lib/dashboard/models';

  interface Props {
    tokens: TokenBreakdown;
  }

  let { tokens }: Props = $props();

  // Cached reads dominate agent traffic, so they get the quiet tone; fresh input and output stand out.
  const parts = $derived([
    { name: 'cache read', value: tokens.cacheRead, color: 'var(--text-3)' },
    { name: 'cache write', value: tokens.cacheWrite, color: 'var(--accent-3)' },
    { name: 'input', value: tokens.input, color: 'var(--accent-2)' },
    { name: 'output', value: tokens.output, color: 'var(--accent-1)' },
  ]);

  const total = $derived(parts.reduce((sum, part) => sum + part.value, 0));

  const offsets = $derived(
    parts.map((part, index) => ({ ...part, start: parts.slice(0, index).reduce((sum, earlier) => sum + earlier.value, 0) })),
  );

  const summary = $derived(parts.map((part) => `${formatTokens(part.value)} ${part.name}`).join(', '));
</script>

<div class="flex flex-col gap-1.5">
  <svg role="img" aria-label="Token mix: {summary}" class="h-2 w-full overflow-hidden rounded-full">
    <rect class="fill-surface-3" width="100%" height="100%" />
    {#if total > 0}
      {#each offsets as part (part.name)}
        <rect x="{(part.start / total) * 100}%" width="{(part.value / total) * 100}%" height="100%" style:fill={part.color} />
      {/each}
    {/if}
  </svg>

  <!-- The key is the label: every segment is named with its exact count, not left to color.
       Two fixed columns keep it two lines tall whatever the counts read, so a range switch never reflows it. -->
  <dl class="tabular grid grid-cols-2 gap-x-3 gap-y-0.5 text-xs leading-4 text-fg-3">
    {#each parts as part (part.name)}
      <div class="flex min-w-0 items-center gap-1.5">
        <span class="size-1.5 shrink-0 rounded-full" style:background={part.color} aria-hidden="true"></span>
        <dt class="sr-only">{part.name}</dt>
        <dd class="truncate" title="{formatTokens(part.value)} {part.name}">{formatTokens(part.value)} {part.name}</dd>
      </div>
    {/each}
  </dl>
</div>
