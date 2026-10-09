<script lang="ts">
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import { updates } from '$lib/stores/updates.svelte';

  import { restartWarning } from './copy';
</script>

<!-- Restarting stops every agent, so a chat in the middle of a turn is confirmed before it is lost. -->
<ConfirmDialog
  bind:open={() => updates.confirmingRestart, (open) => (updates.confirmingRestart = open)}
  title="Restart now?"
  description={restartWarning(updates.chatsAtRisk)}
  confirmLabel={updates.closesApp ? 'Finish update' : 'Restart now'}
  cancelLabel="Keep working"
  onconfirm={() => void updates.relaunch()}
/>
