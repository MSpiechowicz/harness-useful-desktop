<script lang="ts">
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import type { MemoryNotesTarget } from '$lib/ipc/bindings/MemoryNotesTarget';

  import SettingRow from './SettingRow.svelte';

  interface Props {
    title: string;
    description: string;
    /** The team database or this computer. */
    options: StripOption<MemoryNotesTarget>[];
    /** Whether the item is shared through the team database. */
    shared: boolean;
    onchange: (shared: boolean) => void;
  }

  let { title, description, options, shared, onchange }: Props = $props();
</script>

<!-- What the team shares comes from other people and runs on this computer, so a row like this exists only for a server that keeps it. Team database means shared. -->
<SettingRow {title} {description}>
  {#snippet control()}
    <SegmentedControl label={title} {options} bind:value={() => (shared ? 'remote' : 'local'), (next) => onchange(next === 'remote')} />
  {/snippet}
</SettingRow>
