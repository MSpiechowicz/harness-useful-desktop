<script lang="ts">
  import Button from '$lib/components/Button.svelte';
  import { NOTICE_LOOKS } from '$lib/components/notice';
  import { ICON } from '$lib/components/icon';
  import Switch from '$lib/components/Switch.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { uid } from '$lib/utils/ids';

  import { importable, needsDefaultScope } from './notes-import';
  import { notesImport as flow } from './notes-import-state.svelte';
  import NotesImportTable from './NotesImportTable.svelte';
  import ScopeField from './ScopeField.svelte';

  const scopeId = uid('import-scope');
  const look = NOTICE_LOOKS.warning;

  const plan = $derived(flow.plan);
  const projectOpen = $derived(project.active !== null);
  const usable = $derived(flow.rows.filter(importable).length);
  const wantsScope = $derived(needsDefaultScope(flow.rows, flow.picks));
  const scopeHelp = $derived.by(() => {
    if (!projectOpen) {
      return t('memory.import.scope.noProject');
    }

    return flow.defaultScope === 'repo' ? t('memory.import.scope.repo', { project: project.active?.name ?? '' }) : t('memory.import.scope.global');
  });
</script>

{#if plan}
  <div class="flex flex-col gap-4">
    <p class="text-sm text-fg-3">{tn('memory.import.summary', plan.rows.length, { file: plan.fileName, format: t(plan.format === 'csv' ? 'memory.import.format.csv' : 'memory.import.format.json') })}</p>

    {#if plan.truncated}
      <div class="flex items-start gap-3 rounded-md p-3 {look.box}" role="status">
        <span class="flex h-5 shrink-0 items-center"><look.icon class={look.color} size={ICON.sm} aria-hidden="true" /></span>
        <span class="text-sm text-fg">{t('memory.import.truncated')}</span>
      </div>
    {/if}

    <div class="flex items-center justify-between gap-3">
      <p class="tabular text-sm text-fg-2" role="status">{t('memory.import.selected', { picked: flow.picked.length, total: usable })}</p>

      <div class="flex items-center gap-1">
        <Button variant="ghost" disabled={usable === 0 || flow.picked.length === usable} onclick={() => flow.toggleAll(true)}>{t('memory.import.selectAll')}</Button>
        <Button variant="ghost" disabled={flow.picked.length === 0} onclick={() => flow.toggleAll(false)}>{t('memory.import.selectNone')}</Button>
      </div>
    </div>

    <NotesImportTable rows={flow.rows} picks={flow.picks} ontoggle={(index, on) => flow.toggle(index, on)} />

    {#if wantsScope}
      <ScopeField bind:value={flow.defaultScope} fixed={false} {projectOpen} id={scopeId} error={null} help={scopeHelp} />
    {/if}

    <div class="flex items-start justify-between gap-4">
      <div class="flex min-w-0 flex-col gap-0.5">
        <span class="text-sm text-fg">{t('memory.import.replace')}</span>
        <p class="text-xs text-fg-3">{t('memory.import.replace.help')}</p>
      </div>

      <Switch bind:checked={flow.replace} label={t('memory.import.replace')} />
    </div>
  </div>
{/if}
