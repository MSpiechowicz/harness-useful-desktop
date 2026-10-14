<script lang="ts">
  import type { Snippet } from 'svelte';

  import Card from '$lib/components/Card.svelte';
  import StatGrid from '$lib/components/StatGrid.svelte';
  import StatusDot from '$lib/components/StatusDot.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { cx } from '$lib/utils/cx';

  import { shortPath } from '../config/path';
  import type { Fact, Stat } from './extension-detail';

  interface Props {
    title: string;
    /** The kind of item, then one line of what it is: "Plugin · Persistent usage dashboard". */
    subtitle: string;
    description: string | null;
    facts: Fact[];
    /** The numbers of the Overview card; none, and the card is left out. */
    stats?: Stat[];
    /** Kind-specific sections below the description. */
    children?: Snippet;
  }

  let { title, subtitle, description, facts, stats = [], children }: Props = $props();

  /** How many characters at the end of a long value stay visible when its middle is cut. */
  const TAIL = 12;

  const details = $derived(facts.filter((fact) => !fact.location));
  const location = $derived(facts.filter((fact) => fact.location));
</script>

<!-- The Configuration detail: title and content on the left, the item's facts in a column of cards on the right (below the content when narrow). -->
<article class="flex min-h-full flex-col @[760px]:h-full @[760px]:flex-row" aria-labelledby="extension-title">
  <div class="flex min-w-0 flex-1 flex-col @[760px]:min-h-0">
    <header class="shrink-0 border-b border-line px-5 pt-4 pb-3">
      <h2 id="extension-title" class="truncate text-lg leading-6 font-semibold text-fg" title={title}>{title}</h2>
      <p class="mt-0.5 truncate text-xs text-fg-3" title={subtitle}>{subtitle}</p>
    </header>

    <div class="flex min-h-0 flex-1 flex-col gap-4 px-5 pt-4 pb-6 @[760px]:overflow-y-auto">
      {#if description}
        <Card as="section" padded={false} class="p-3.5">
          <h3 class="mb-2.5 text-xs font-medium tracking-wider text-fg-3 uppercase">{t('extensions.detail.description')}</h3>
          <p class="text-sm leading-relaxed break-words text-fg-2">{description}</p>
        </Card>
      {/if}

      <!-- Identifiers and paths are long, so they get the width of the content, not of the narrow column. -->
      {@render facts_card(t('extensions.detail.location'), location, false)}

      {@render children?.()}
    </div>
  </div>

  <aside
    aria-label={t('extensions.detail.aside')}
    class="flex shrink-0 flex-col gap-3 border-t border-line p-4 @[760px]:w-[280px] @[760px]:overflow-y-auto @[760px]:border-t-0 @[760px]:border-l"
  >
    {#if stats.length > 0}
      <Card as="section" padded={false} class="p-3.5">
        <h3 class="mb-2.5 text-xs font-medium tracking-wider text-fg-3 uppercase">{t('extensions.overview.title')}</h3>
        <StatGrid stats={stats} />
      </Card>
    {/if}

    {@render facts_card(t('extensions.detail.details'), details, false)}
  </aside>
</article>

{#snippet facts_card(heading: string, rows: Fact[], shorten: boolean)}
  {#if rows.length > 0}
    <Card as="section" padded={false} class="p-3.5">
      <h3 class="mb-2.5 text-xs font-medium tracking-wider text-fg-3 uppercase">{heading}</h3>
      <dl class="flex flex-col">
        {#each rows as fact (fact.label)}
          {@const text = shorten && fact.path ? shortPath(fact.value) : fact.value}
          <!-- Every row is the label on the left and the value on the right, on one line. A long value is shortened in the
               middle, so both its start and its end (the version, the file name) stay readable; the tooltip has all of it. -->
          <div class="flex justify-between gap-3 py-1 text-sm">
            <dt class="shrink-0 text-fg-2">{fact.label}</dt>
            <dd class={cx('flex min-w-0 items-center justify-end gap-1.5 text-right', fact.on ? 'text-success' : 'text-fg')} title={fact.title ?? fact.value}>
              {#if fact.on}<StatusDot tone="success" />{/if}
              {#if fact.mono && text.length > TAIL}
                <span class="flex min-w-0 font-mono text-xs leading-5"><span class="truncate">{text.slice(0, -TAIL)}</span><span class="shrink-0">{text.slice(-TAIL)}</span></span>
              {:else}
                <span class={cx('truncate', fact.mono && 'font-mono text-xs leading-5')}>{text}</span>
              {/if}
            </dd>
          </div>
        {/each}
      </dl>
    </Card>
  {/if}
{/snippet}
