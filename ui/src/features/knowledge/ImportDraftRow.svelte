<script lang="ts">
  import Switch from '$lib/components/Switch.svelte';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { knowledgeImport as flow } from '$lib/stores/knowledgeImport.svelte';
  import { uid } from '$lib/utils/ids';

  import { showTeamApproval } from '../settings/approval-focus.svelte';

  const descriptionId = uid('import-draft');

  /** Closes the import window (what is staged stays) and opens Settings at the team approval row. */
  function goToSettings(): void {
    flow.close();
    showTeamApproval();
  }

  const locked = $derived(flow.approvalLocked);
  const reason = $derived(t('knowledge.drafts.import.locked'));
</script>

<!-- A plain row under the files, told apart by its hairline: a switch and what it does. With team approval on every document starts as a draft, so the switch is on and cannot
     be changed, and the row says why with a way to the setting. -->
<div class="flex items-center gap-6 border-t border-line pt-4">
  <div class="flex min-w-0 flex-1 flex-col gap-0.5">
    <p class="text-base font-medium text-fg">{t('knowledge.drafts.import.title')}</p>

    <p id={descriptionId} class="text-sm text-fg-3">
      {#if locked}
        {reason}
        <button type="button" class="rounded-xs text-fg-2 underline decoration-line-strong underline-offset-3 transition-colors duration-[var(--dur-fast)] hover:text-fg hover:decoration-current" onclick={goToSettings}>{t('knowledge.drafts.import.settings')}</button>
      {:else}
        {t('knowledge.drafts.import.hint')}
      {/if}
    </p>
  </div>

  <Tooltip text={locked ? reason : ''} disabled={!locked} side="top">
    {#snippet children({ describedby })}
      <span aria-describedby={locked ? describedby : undefined} class="inline-flex">
        <Switch label={t('knowledge.drafts.import.title')} disabled={locked} bind:checked={() => flow.asDraft, (next) => (flow.draft = next)} />
      </span>
    {/snippet}
  </Tooltip>
</div>
