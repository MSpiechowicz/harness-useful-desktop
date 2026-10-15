<script lang="ts">
  import Download from '@lucide/svelte/icons/download';
  import Upload from '@lucide/svelte/icons/upload';

  import { t } from '$lib/i18n/i18n.svelte';
  import { uid } from '$lib/utils/ids';

  import SyncCard from './SyncCard.svelte';
  import SyncExportDialog from './SyncExportDialog.svelte';
  import SyncImportDialog from './SyncImportDialog.svelte';

  interface Props {
    id?: string;
  }

  let { id }: Props = $props();

  const headingId = uid('settings-heading');

  let exporting = $state(false);
  let importing = $state(false);
</script>

<section {id} aria-labelledby={headingId} class="flex scroll-mt-6 flex-col gap-3" data-settings-section={id}>
  <h2 id={headingId} class="px-1 text-sm font-medium tracking-wider text-fg-3 uppercase">{t('settings.section.sync')}</h2>

  <div class="grid gap-4 @xl:grid-cols-2 @xl:grid-rows-[auto_auto_auto_auto]">
    <SyncCard
      icon={Upload}
      title={t('sync.export.title')}
      steps={[t('sync.export.flow.1'), t('sync.export.flow.2')]}
      does={t('sync.export.card.does')}
      never={t('sync.export.card.never')}
      action={t('sync.export.action')}
      hint={t('sync.export.hint')}
      onaction={() => (exporting = true)}
    />
    <SyncCard
      icon={Download}
      title={t('sync.import.title')}
      steps={[t('sync.import.flow.1'), t('sync.import.flow.2'), t('sync.import.flow.3')]}
      does={t('sync.import.card.does')}
      never={t('sync.import.card.never')}
      action={t('sync.import.action')}
      hint={t('sync.import.hint')}
      onaction={() => (importing = true)}
    />
  </div>
</section>

<SyncExportDialog bind:open={exporting} />
<SyncImportDialog bind:open={importing} />
