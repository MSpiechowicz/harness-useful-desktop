<script lang="ts">
  import ShieldCheck from '@lucide/svelte/icons/shield-check';

  import Badge from '$lib/components/Badge.svelte';
  import Logo from '$lib/components/Logo.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { app } from '$lib/stores/app.svelte';
</script>

<section aria-label="About" class="flex items-center gap-5 rounded-lg border border-line bg-surface-1 p-5 shadow-raised">
  <Logo size={56} />

  <div class="flex min-w-0 flex-1 flex-col gap-1">
    {#if app.info}
      <h2 class="text-xl text-fg">{app.info.name}</h2>
      <p class="tabular text-sm text-fg-2">Version {app.info.version} · {app.info.os} {app.info.arch}{app.info.debug ? ' · debug build' : ''}</p>
    {:else}
      <Skeleton width="10rem" height="1.5rem" />
      <Skeleton width="14rem" height="0.875rem" />
    {/if}
  </div>

  <Badge tone="success"><ShieldCheck size={12} aria-hidden="true" />Local only</Badge>
</section>
