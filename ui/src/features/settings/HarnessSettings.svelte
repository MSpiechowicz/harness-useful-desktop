<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import Button from '$lib/components/Button.svelte';
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';

  import SettingRow from './SettingRow.svelte';

  function resolvedPath(summary: HarnessSummary): string {
    return summary.detection.binaryPath ?? t('settings.harness.notFound');
  }
</script>

{#each harness.summaries as summary (summary.info.id)}
  <SettingRow title={t('settings.harness.binary', { name: summary.info.name })} description={resolvedPath(summary)}>
    <!-- One width for every harness, so the titles line up. -->
    {#snippet leading()}<HarnessBadge info={summary.info} class="w-12" />{/snippet}
    {#snippet control()}
      <div class="flex items-center gap-2">
        <Button onclick={() => void harness.pickBinary(summary.info.id)}>{t('settings.harness.choose')}</Button>
        {#if harness.hasOverride(summary.info.id)}
          <Button variant="ghost" onclick={() => void harness.clearBinary(summary.info.id)}>{t('settings.harness.reset')}</Button>
        {/if}
      </div>
    {/snippet}
  </SettingRow>
{/each}
