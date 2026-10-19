<script lang="ts">
  import type { ComponentProps } from 'svelte';

  import Badge from '../Badge.svelte';
  import Tooltip from '../Tooltip.svelte';

  interface Props {
    label: string;
    tone: NonNullable<ComponentProps<typeof Badge>['tone']>;
    /** A live dot, for the state that runs. */
    dot?: boolean;
    /** Says why the state is what it is (an overridden team document): shown as the badge's tooltip. */
    hint?: string;
  }

  let { label, tone, dot = false, hint }: Props = $props();
</script>

{#if hint}
  <Tooltip text={hint}>
    {#snippet children({ describedby })}
      <span aria-describedby={describedby} class="inline-flex"><Badge {tone} size="lg">{label}</Badge></span>
    {/snippet}
  </Tooltip>
{:else}
  <Badge {tone} {dot} size="lg">{label}</Badge>
{/if}
