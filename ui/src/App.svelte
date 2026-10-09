<script lang="ts">
  import { onMount } from 'svelte';

  import ErrorBoundary from '$lib/components/ErrorBoundary.svelte';
  import ToastHost from '$lib/components/ToastHost.svelte';
  import { app } from '$lib/stores/app.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { motion } from '$lib/stores/motion.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { project } from '$lib/stores/project.svelte';

  import AuroraBackdrop from './features/intro/AuroraBackdrop.svelte';
  import Intro from './features/intro/Intro.svelte';
  import { intro } from './features/intro/intro.svelte';
  import Onboarding from './features/onboarding/Onboarding.svelte';
  import Shell from './features/shell/Shell.svelte';

  onMount(() => {
    void start();
  });

  /** Cold start: load preferences and app info, then play the intro if the user wants it. */
  async function start(): Promise<void> {
    await Promise.all([prefs.load(), app.load()]);

    // Detection can take seconds; it must not hold the intro back, so these run alongside it.
    void harness.load();
    void project.load();

    const wantsIntro = prefs.value?.introEnabled ?? false;
    intro.play(wantsIntro ? motion.level : 'off');
  }
</script>

<!-- Each layer fails on its own: decorative ones disappear, the shell and the setup show "Something went wrong". -->
<ErrorBoundary area="The background" variant="silent"><AuroraBackdrop /></ErrorBoundary>
<ErrorBoundary area="Harness Desktop" variant="screen"><Shell /></ErrorBoundary>
<ErrorBoundary area="The setup" variant="screen"><Onboarding /></ErrorBoundary>
<ErrorBoundary area="The intro" variant="silent"><Intro /></ErrorBoundary>
<ErrorBoundary area="Notifications" variant="silent"><ToastHost /></ErrorBoundary>
