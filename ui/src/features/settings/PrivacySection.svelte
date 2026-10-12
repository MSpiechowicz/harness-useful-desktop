<script lang="ts">
  import ClipboardCopy from '@lucide/svelte/icons/clipboard-copy';
  import Trash from '@lucide/svelte/icons/trash-2';

  import Button from '$lib/components/Button.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import Switch from '$lib/components/Switch.svelte';
  import { app } from '$lib/stores/app.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';

  import { diagnosticsText } from './diagnostics';
  import { PRIVACY_FACTS } from './privacy-facts';
  import SettingRow from './SettingRow.svelte';
  import SettingsSection from './SettingsSection.svelte';

  interface Props {
    id?: string;
  }

  let { id }: Props = $props();

  let confirmClear = $state(false);

  async function clearRecents(): Promise<void> {
    for (const recent of [...project.recents]) {
      await project.forget(recent.path);
    }

    if (project.recents.length === 0) {
      toasts.push({ title: t('settings.privacy.recents.cleared'), tone: 'success' });
    }
  }

  async function copyDiagnostics(): Promise<void> {
    try {
      await navigator.clipboard.writeText(diagnosticsText(app.info, harness.summaries));
    } catch {
      toasts.push({ title: t('settings.privacy.diagnostics.failed'), description: t('settings.privacy.diagnostics.failedDescription'), tone: 'warning' });

      return;
    }

    toasts.push({ title: t('settings.privacy.diagnostics.copied'), description: t('settings.privacy.diagnostics.copiedDescription'), tone: 'success' });
  }
</script>

<SettingsSection {id} title={t('settings.section.privacy')}>
  <ul class="flex flex-col gap-4 pb-4" aria-label={t('settings.privacy.label')}>
    {#each PRIVACY_FACTS as fact (fact.id)}
      <li class="flex items-start gap-3">
        <span class="mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-md bg-surface-2 text-fg-2">
          <fact.icon size={16} aria-hidden="true" />
        </span>

        <div class="flex min-w-0 flex-col gap-0.5">
          <p class="text-base font-medium text-fg">{fact.title}</p>
          <p class="text-sm text-fg-2">{fact.text}</p>
        </div>
      </li>
    {/each}
  </ul>

  <SettingRow
    title={t('settings.privacy.openLink.title')}
    description={t('settings.privacy.openLink.description')}
  >
    {#snippet control()}
      <Switch label={t('settings.privacy.openLink.title')} bind:checked={() => prefs.value?.openAnyLink ?? false, (openAnyLink) => void prefs.update({ openAnyLink })} />
    {/snippet}
  </SettingRow>

  <SettingRow title={t('settings.privacy.recents.title')} description={t('settings.privacy.recents.description')}>
    {#snippet control()}
      <Button variant="dangerQuiet" disabled={project.recents.length === 0} onclick={() => (confirmClear = true)}>
        {#snippet leading()}<Trash size={16} aria-hidden="true" />{/snippet}
        {t('settings.privacy.recents.clear')}
      </Button>
    {/snippet}
  </SettingRow>

  <SettingRow title={t('settings.privacy.diagnostics.title')} description={t('settings.privacy.diagnostics.description')}>
    {#snippet control()}
      <Button onclick={() => void copyDiagnostics()}>
        {#snippet leading()}<ClipboardCopy size={16} aria-hidden="true" />{/snippet}
        {t('settings.privacy.diagnostics.copy')}
      </Button>
    {/snippet}
  </SettingRow>
</SettingsSection>

<ConfirmDialog
  bind:open={confirmClear}
  title={t('settings.privacy.recents.confirmTitle')}
  description={t('settings.privacy.recents.confirmDescription')}
  confirmLabel={t('settings.privacy.recents.confirmLabel')}
  onconfirm={() => void clearRecents()}
/>
