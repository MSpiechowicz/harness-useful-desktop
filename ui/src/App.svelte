<script lang="ts">
  import { onMount } from 'svelte';

  import ToastHost from '$lib/components/ToastHost.svelte';
  import { app } from '$lib/stores/app.svelte';
  import { motion } from '$lib/stores/motion.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';

  import AuroraBackdrop from './features/intro/AuroraBackdrop.svelte';
  import Intro from './features/intro/Intro.svelte';
  import { intro } from './features/intro/intro.svelte';
  import Shell from './features/shell/Shell.svelte';

  onMount(() => {
    void start();
  });

  /** Cold start: load preferences and app info, then play the intro if the user wants it. */
  async function start(): Promise<void> {
    await Promise.all([prefs.load(), app.load()]);

    const wantsIntro = prefs.value?.introEnabled ?? false;
    intro.play(wantsIntro ? motion.level : 'off');
  }
</script>

<AuroraBackdrop />
<Shell />
<Intro />
<ToastHost />
