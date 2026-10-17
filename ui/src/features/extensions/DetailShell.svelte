<script lang="ts">
  import type { Snippet } from 'svelte';

  import FactsCard from '$lib/components/FactsCard.svelte';
  import SectionCard from '$lib/components/SectionCard.svelte';
  import StatGrid from '$lib/components/StatGrid.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

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

  const details = $derived(facts.filter((fact) => !fact.location));
  const location = $derived(facts.filter((fact) => fact.location));
</script>

<!-- The Configuration detail: title and content on the left, the item's facts in a column of cards on the right; the column stays beside the content at every width. -->
<article class="flex h-full flex-row" aria-labelledby="extension-title">
  <div class="flex min-h-0 min-w-0 flex-1 flex-col">
    <header class="shrink-0 border-b border-line px-5 pt-4 pb-3">
      <h2 id="extension-title" class="truncate text-lg leading-6 font-semibold text-fg" title={title}>{title}</h2>
      <p class="mt-0.5 truncate text-xs text-fg-3" title={subtitle}>{subtitle}</p>
    </header>

    <div class="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-5 pt-4 pb-6">
      {#if description}
        <SectionCard heading={t('extensions.detail.description')}>
          <p class="text-sm leading-relaxed break-words text-fg-2">{description}</p>
        </SectionCard>
      {/if}

      <!-- Identifiers and paths are long, so they get the width of the content, not of the narrow column. -->
      <FactsCard heading={t('extensions.detail.location')} rows={location} />

      {@render children?.()}
    </div>
  </div>

  <aside aria-label={t('extensions.detail.aside')} class="flex w-[280px] shrink-0 flex-col gap-3 overflow-y-auto border-l border-line p-4">
    {#if stats.length > 0}
      <SectionCard heading={t('extensions.overview.title')}>
        <StatGrid stats={stats} />
      </SectionCard>
    {/if}

    <FactsCard heading={t('extensions.detail.details')} rows={details} />
  </aside>
</article>
