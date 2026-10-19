<script lang="ts" generics="T extends string">
  import { uid } from '$lib/utils/ids';

  import OptionStrip from './OptionStrip.svelte';
  import type { StripOption } from './strip';

  interface Props {
    options: StripOption<T>[];
    value: T;
    label: string;
    onchange?: (value: T) => void;
    /** Every segment as wide as the widest, and `class` can give the whole group a width. */
    equal?: boolean;
    class?: string;
  }

  let { options, value = $bindable(), label, onchange, equal, class: className }: Props = $props();
</script>

<OptionStrip {options} bind:value kind="radio" appearance="pill" {label} idBase={uid('segmented')} {onchange} {equal} class={className} />
