<script lang="ts" generics="T extends string">
  import { uid } from '$lib/utils/ids';

  import OptionStrip from './OptionStrip.svelte';
  import type { StripOption } from './strip';

  interface Props {
    options: StripOption<T>[];
    value: T;
    label: string;
    onchange?: (value: T) => void;
  }

  let { options, value = $bindable(), label, onchange }: Props = $props();
</script>

<OptionStrip {options} bind:value kind="radio" appearance="pill" {label} idBase={uid('segmented')} {onchange} />
