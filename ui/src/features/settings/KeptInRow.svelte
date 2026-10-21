<script lang="ts">
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import type { MemoryNotesRouting } from '$lib/ipc/bindings/MemoryNotesRouting';

  import SettingRow from './SettingRow.svelte';

  interface Props {
    title: string;
    description: string;
    /** Every text `description` can switch to; the row keeps room for the longest. */
    descriptions?: string[];
    /** The three places, the same in every row: the ones the row cannot do are greyed out. */
    options: StripOption<MemoryNotesRouting>[];
    value: MemoryNotesRouting;
    /** The line under the control for each place. */
    hints: Record<MemoryNotesRouting, string>;
    onchange: (value: MemoryNotesRouting) => void;
  }

  let { title, description, descriptions, options, value, hints, onchange }: Props = $props();
</script>

<!-- One row of "Kept in": the same control, width and options in every row, and one line under it that says what the chosen place does. -->
<SettingRow {title} {description} {descriptions}>
  {#snippet control()}
    <!-- The segments take the width of the longest label (the same three in every row), so every row lines up in any language. -->
    <SegmentedControl label={title} {options} equal bind:value={() => value, onchange} />
  {/snippet}
  {#snippet below()}
    <!-- The hint hangs under the control, which stays centred on the title and description. It wraps at the control's width instead of widening it, and keeps room for the longest hint, so choosing a place never shifts the layout. -->
    <p class="mt-1.5 grid w-0 min-w-full text-xs text-fg-3">
      {#each Object.entries(hints) as [place, text] (place)}
        <span class="invisible [grid-area:1/1]" aria-hidden="true">{text}</span>
      {/each}
      <span class="[grid-area:1/1]">{hints[value]}</span>
    </p>
  {/snippet}
</SettingRow>
