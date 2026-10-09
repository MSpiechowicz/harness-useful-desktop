<svelte:options namespace="svg" />

<script lang="ts">
  import { ACCENT_STOPS, GRADIENT_AXIS } from './geometry';

  interface Props {
    id: string;
    /** SVG transform of the gradient itself, e.g. to turn it with a rotating shape. */
    transform?: string;
  }

  let { id, transform }: Props = $props();
</script>

<!-- Defined inside each SVG that uses it: a shared external gradient would not see the theme's CSS variables. -->
<linearGradient {id} gradientUnits="userSpaceOnUse" gradientTransform={transform} {...GRADIENT_AXIS}>
  {#each ACCENT_STOPS as stop (stop.offset)}
    <stop offset={stop.offset} style:stop-color={stop.color} />
  {/each}
</linearGradient>
