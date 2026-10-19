<script lang="ts">
  interface Props {
    stats: { key: string; label: string; value: number }[];
  }

  let { stats }: Props = $props();
</script>

<!-- The same figures in the review and in the result of a copy: the number large, its name small beneath it. -->
<dl class="grid grid-cols-2 gap-3 {stats.length > 4 ? '@lg:grid-cols-5' : '@lg:grid-cols-4'}">
  {#each stats as stat (stat.key)}
    <div class="flex min-w-0 flex-col-reverse">
      <dt class="truncate text-xs text-fg-3" title={stat.label}>{stat.label}</dt>
      <dd class="tabular truncate text-lg font-semibold text-fg">{stat.value}</dd>
    </div>
  {/each}
</dl>
