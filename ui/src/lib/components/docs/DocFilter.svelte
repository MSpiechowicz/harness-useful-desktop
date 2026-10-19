<script lang="ts" generics="V extends string">
  import FilterMenu from '../FilterMenu.svelte';
  import type { IconComponent } from '../icon';

  interface Props {
    /** What the filter narrows by ("When"): the button reads "When: All". */
    label: string;
    icon?: IconComponent;
    /** The first option is the unfiltered "All". */
    options: { value: V; label: string; icon?: IconComponent; count?: number }[];
    value: V;
    onchange: (value: V) => void;
    /** The accessible name of the clear x, e.g. "Clear the When filter". */
    clearLabel: string;
  }

  let { label, icon, options, value, onchange, clearLabel }: Props = $props();
</script>

<!-- A filter of a documents list: its button names what it narrows by, and its x takes it back to the first option. -->
<FilterMenu {label} prefix={label} {icon} {options} {value} {onchange} onclear={() => onchange(options[0].value)} {clearLabel} align="start" />
