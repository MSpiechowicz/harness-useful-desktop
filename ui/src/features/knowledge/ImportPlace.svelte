<script lang="ts">
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { KnowledgePlace } from '$lib/ipc/bindings/KnowledgePlace';
  import type { KnowledgeRemote } from '$lib/ipc/bindings/KnowledgeRemote';
  import { effectivePlace, placesFor } from '$lib/utils/knowledge-routing';

  import { placeLabel } from './knowledge-format';

  interface Props {
    /** The saved routing. */
    remote: KnowledgeRemote;
    /** Whether the documents link to no repository, which makes them global. */
    global: boolean;
    /** The user's choice, none for the routing's default for that kind. */
    place: KnowledgePlace | null;
    onchange: (place: KnowledgePlace) => void;
  }

  let { remote, global, place, onchange }: Props = $props();

  /** Where this kind of document may go under the routing, and where it goes unless chosen. */
  const rule = $derived(placesFor(global ? remote.globalDocuments : remote.repositoryDocuments));
  /** A choice made for the other kind of document does not carry over: the routing's default stands until the user chooses again. */
  const value = $derived(effectivePlace(remote, global, place));
  const options = $derived<StripOption<KnowledgePlace>[]>(
    (['remote', 'local'] as const).map((each) => {
      const allowed = rule.allowed.includes(each);

      return { value: each, label: placeLabel(each), disabled: !allowed, ...(allowed ? {} : { reason: t('knowledge.import.saveTo.notAllowed') }) };
    }),
  );
</script>

<!-- Only shown while a kind of document reads Remote + this computer: with one place there is nothing to choose. -->
<div class="flex flex-wrap items-center justify-between gap-3">
  <div class="min-w-0">
    <p class="text-base font-medium text-fg">{t('knowledge.import.saveTo.label')}</p>
    <p class="text-sm text-fg-3">{t('knowledge.import.saveTo.hint')}</p>
  </div>
  <SegmentedControl label={t('knowledge.import.saveTo.label')} {options} equal bind:value={() => value, onchange} />
</div>
