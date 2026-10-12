<script lang="ts">
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { updates } from '$lib/stores/updates.svelte';

  import { restartWarning } from './copy';
</script>

<!-- Restarting stops every agent, so a chat in the middle of a turn is confirmed before it is lost. -->
<ConfirmDialog
  bind:open={() => updates.confirmingRestart, (open) => (updates.confirmingRestart = open)}
  title={t('updates.restart.title')}
  description={restartWarning(updates.chatsAtRisk)}
  confirmLabel={updates.closesApp ? t('updates.restart.finish') : t('updates.restart.confirm')}
  cancelLabel={t('updates.restart.keepWorking')}
  onconfirm={() => void updates.relaunch()}
/>
