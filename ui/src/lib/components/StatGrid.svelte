<script lang="ts">
  interface Props {
    stats: { label: string; value: string }[];
  }

  let { stats }: Props = $props();
</script>

<!-- The numbers of an Overview card: two columns, the figure large and its name small beneath it. -->
<dl class="grid grid-cols-2 gap-x-3 gap-y-3">
  {#each stats as stat (stat.label)}
    <div class="flex min-w-0 flex-col-reverse">
      <dt class="truncate text-xs text-fg-3">{stat.label}</dt>
      <dd class="tabular truncate text-lg font-semibold text-fg" title={stat.value}>{stat.value}</dd>
    </div>
  {/each}
</dl>
