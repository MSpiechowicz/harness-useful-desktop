<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { harnessUpdates } from '$lib/stores/harness-updates.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import Button from '$lib/components/Button.svelte';
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import Switch from '$lib/components/Switch.svelte';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';

  import SettingRow from './SettingRow.svelte';

  function resolvedPath(summary: HarnessSummary): string {
    return summary.detection.binaryPath ?? t('settings.harness.notFound');
  }

  function updateLabel(version: string): string {
    return t('ecosystem.row.updateTo', { version });
  }

  // Every Update button keeps room for the longest of these, so the buttons of all rows are one width.
  const updateLabels = $derived([...new Set(harness.summaries.flatMap((summary) => harnessUpdates.offer(summary.info.id)?.latestVersion ?? []))].map(updateLabel));
</script>

{#each harness.summaries as summary (summary.info.id)}
  <SettingRow title={t('settings.harness.binary', { name: summary.info.name })} description={resolvedPath(summary)}>
    <!-- One width for every harness, so the titles line up. -->
    {#snippet leading()}<HarnessBadge info={summary.info} class="w-12" />{/snippet}
    {#snippet control()}
      {@const newer = harnessUpdates.offer(summary.info.id)}
      <div class="flex items-center gap-2">
        {#if newer}
          <!-- While it runs the button says why it cannot be pressed again. -->
          <Tooltip text={t('settings.harness.updating', { name: summary.info.name })} disabled={!harnessUpdates.isUpdating(summary.info.id)}>
            {#snippet children({ describedby })}
              <Button
                variant="primary"
                disabled={harnessUpdates.isUpdating(summary.info.id)}
                aria-label={t('settings.harness.updateAria', { name: summary.info.name, version: newer.latestVersion })}
                aria-describedby={describedby}
                onclick={() => void harnessUpdates.update(summary.info.id)}
                bare
              >
                <span class="grid justify-items-center">
                  {#each updateLabels as label (label)}
                    <span class="label-trim invisible [grid-area:1/1]" aria-hidden="true">{label}</span>
                  {/each}
                  <span class="label-trim [grid-area:1/1]">{updateLabel(newer.latestVersion)}</span>
                </span>
              </Button>
            {/snippet}
          </Tooltip>
        {/if}
        <Button onclick={() => void harness.pickBinary(summary.info.id)}>{t('settings.harness.choose')}</Button>
        {#if harness.hasOverride(summary.info.id)}
          <Button variant="ghost" onclick={() => void harness.clearBinary(summary.info.id)}>{t('settings.harness.reset')}</Button>
        {/if}
      </div>
    {/snippet}
  </SettingRow>
{/each}

<!-- The npm lookup behind the Update buttons and the offer toast (harness_releases): off means no request at all. -->
<SettingRow title={t('settings.harness.checkUpdates.title')} description={t('settings.harness.checkUpdates.description')}>
  {#snippet control()}
    <Switch label={t('settings.harness.checkUpdates.title')} disabled={prefs.value === null} bind:checked={() => prefs.value?.checkHarnessUpdates ?? false, (checkHarnessUpdates) => void prefs.update({ checkHarnessUpdates })} />
  {/snippet}
</SettingRow>
