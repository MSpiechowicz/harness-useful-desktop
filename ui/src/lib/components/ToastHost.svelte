<script lang="ts">
  import { toasts } from '$lib/stores/toasts.svelte';

  import Toast from './Toast.svelte';
</script>

<!-- Polite live region: individual toasts announce themselves via role=status/alert. -->
<section aria-label="Notifications" class="pointer-events-none fixed right-4 bottom-12 z-[90] flex flex-col items-end gap-2">
  {#each toasts.items as toast (toast.id)}
    <Toast {toast} ondismiss={(id) => toasts.dismiss(id)} />
  {/each}
</section>
