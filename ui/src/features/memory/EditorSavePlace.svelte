<script lang="ts">
  import DocField from '$lib/components/docs/DocField.svelte';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MemorySavePlace } from '$lib/ipc/bindings/MemorySavePlace';

  interface Props {
    value: MemorySavePlace;
    /** The id of the label. */
    id: string;
    /** The remote cannot be reached now: only this computer can be chosen, and the field says why. */
    offline: boolean;
  }

  let { value = $bindable(), id, offline }: Props = $props();

  const options = $derived<StripOption<MemorySavePlace>[]>([
    { value: 'remote', label: t('memory.editor.place.remote'), disabled: offline, reason: offline ? t('memory.editor.place.offline') : undefined },
    { value: 'local', label: t('memory.editor.place.local') },
  ]);

  const help = $derived.by(() => {
    if (offline) {
      return t('memory.editor.place.offline');
    }

    return t(value === 'remote' ? 'memory.editor.place.remote.help' : 'memory.editor.place.local.help');
  });
</script>

<!-- Where a new entry of a scope kept in "Remote + this computer" goes: the remote database or this computer. -->
<DocField label={t('memory.editor.place.label')} labelId={id} {help}>
  <div class="flex"><SegmentedControl label={t('memory.editor.place.label')} {options} bind:value /></div>
</DocField>
