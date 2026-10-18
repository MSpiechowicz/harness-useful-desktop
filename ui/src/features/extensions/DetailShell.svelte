<script lang="ts">
  import type { Snippet } from 'svelte';

  import FactsCard from '$lib/components/FactsCard.svelte';
  import SectionCard from '$lib/components/SectionCard.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import type { Fact } from './extension-detail';

  interface Props {
    description: string | null;
    facts: Fact[];
    /** Kind-specific sections below the description. */
    children?: Snippet;
  }

  let { description, facts, children }: Props = $props();

  const location = $derived(facts.filter((fact) => fact.location));
</script>

<!-- The centre of the Extensions view: description, location and the kind-specific sections, flat on the page. The title is the header of
     the view (`WorkspaceLayout` puts it above this), and the item's stats and facts are `ExtensionAside`, in the details panel. -->
<article class="flex flex-col gap-4" aria-labelledby="extension-title">
  {#if description}
    <SectionCard heading={t('extensions.detail.description')}>
      <p class="text-sm leading-relaxed break-words text-fg-2">{description}</p>
    </SectionCard>
  {/if}

  <FactsCard heading={t('extensions.detail.location')} rows={location} />

  {@render children?.()}
</article>
