<script lang="ts">
  import WifiOff from '@lucide/svelte/icons/wifi-off';

  import { rise } from '$lib/design/motion';
  import { app } from '$lib/stores/app.svelte';
</script>

{#if app.status === 'unavailable'}
  <div
    role="alert"
    transition:rise={{ y: -8 }}
    class="mx-6 mt-4 flex items-start gap-3 rounded-lg border border-line-strong bg-warning-wash p-4 text-base text-fg"
  >
    <WifiOff class="mt-0.5 shrink-0 text-warning" size={18} aria-hidden="true" />
    <div class="flex flex-col gap-0.5">
      <p class="font-medium">Running without the desktop runtime</p>
      <p class="text-fg-2">
        {app.failure?.message ?? 'The backend did not respond.'}
      </p>
    </div>
  </div>
{/if}
