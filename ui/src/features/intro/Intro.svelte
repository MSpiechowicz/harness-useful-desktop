<script lang="ts">
  import Logo from '$lib/components/Logo.svelte';
  import { motion } from '$lib/stores/motion.svelte';

  import { intro } from './intro.svelte';

  // Only the full-motion intro shows the logo; reduced motion is a plain crossfade of the shell.
  const visible = $derived(motion.level === 'full' && intro.active);
  const leaving = $derived(intro.phase === 'handoff');

  function skip(): void {
    if (intro.active) {
      intro.skip();
    }
  }
</script>

<!-- Any key or click skips instantly; the intro never outlasts its 1.6 s timeline. -->
<svelte:window onkeydown={skip} onpointerdown={skip} />

{#if visible}
  <div
    class="pointer-events-none fixed inset-0 z-[100] grid place-items-center transition-[opacity,transform] duration-[700ms] ease-[var(--ease-out-expo)]"
    class:opacity-0={leaving}
    class:scale-110={leaving}
    aria-hidden="true"
  >
    <Logo size={120} wordmark layout="stacked" animate />
  </div>
{/if}
