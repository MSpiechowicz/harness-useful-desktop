<script lang="ts">
  import { onMount } from 'svelte';

  import ErrorBoundary from '$lib/components/ErrorBoundary.svelte';
  import ToastHost from '$lib/components/ToastHost.svelte';
  import { app } from '$lib/stores/app.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { motion } from '$lib/stores/motion.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { project } from '$lib/stores/project.svelte';

  import Intro from './features/intro/Intro.svelte';
  import { intro } from './features/intro/intro-state.svelte';
  import Onboarding from './features/onboarding/Onboarding.svelte';
  import Shell from './features/shell/Shell.svelte';

  onMount(() => {
    void start();
  });

  /** Cold start: load preferences and app info, then play the intro if the user wants it. */
  async function start(): Promise<void> {
    // main.ts usually has the preferences already: it waits for them to pick the language.
    await Promise.all([prefs.status === 'ready' ? Promise.resolve() : prefs.load(), app.load()]);

    // Detection can take seconds; it must not hold the intro back, so these run alongside it.
    void harness.load();
    void project.load();

    const wantsIntro = prefs.value?.introEnabled ?? false;
    intro.play(wantsIntro ? motion.level : 'off');
  }
</script>

<!-- Each layer fails on its own: decorative ones disappear, the shell and the setup show "Something went wrong". -->
<ErrorBoundary area="Harness Desktop" variant="screen"><Shell /></ErrorBoundary>
<ErrorBoundary area={t('harness.boundary.setup')} variant="screen"><Onboarding /></ErrorBoundary>
<ErrorBoundary area={t('harness.boundary.intro')} variant="silent"><Intro /></ErrorBoundary>
<ErrorBoundary area={t('harness.boundary.notifications')} variant="silent"><ToastHost /></ErrorBoundary>
