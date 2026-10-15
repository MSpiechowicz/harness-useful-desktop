<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import Checkbox from '$lib/components/Checkbox.svelte';
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
  import { harness as harnesses } from '$lib/stores/harness.svelte';

  import type { Chip } from './sync-model';

  interface Props {
    harness: HarnessId;
    /** A short line under the name, such as how many items are ticked. */
    meta?: string;
    chip?: Chip;
    /** Ticks or unticks everything of the group at once; absent when nothing in it can be chosen. */
    all?: { checked: boolean; onchange: (on: boolean) => void };
    id: string;
  }

  let { harness: id, meta, chip, all, id: headingId }: Props = $props();

  const info = $derived(harnesses.get(id)?.info ?? { id, shortName: id.toUpperCase() });
  const name = $derived(harnesses.get(id)?.info.name ?? id.toUpperCase());
</script>

<div class="flex items-center gap-2.5 pb-2">
  <HarnessBadge {info} size="tile" />

  <div class="min-w-0 flex-1">
    <h3 id={headingId} class="truncate text-base leading-5 font-medium text-fg" title={name}>{name}</h3>
    {#if meta}
      <p class="truncate text-xs leading-4 text-fg-3">{meta}</p>
    {/if}
  </div>

  {#if chip}
    <Badge size="lg" tone={chip.tone}>{chip.label}</Badge>
  {/if}

  {#if all}
    <span class="flex items-center gap-1 text-sm text-fg-2">
      {t('sync.group.selectAll')}
      <Checkbox label={t('sync.group.selectAllFor', { name })} checked={all.checked} onchange={all.onchange} />
    </span>
  {/if}
</div>
