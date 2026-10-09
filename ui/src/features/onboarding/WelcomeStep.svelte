<script lang="ts">
  import Eye from '@lucide/svelte/icons/eye';
  import HardDrive from '@lucide/svelte/icons/hard-drive';
  import ShieldCheck from '@lucide/svelte/icons/shield-check';

  import { autofocus } from '$lib/a11y/autofocus';
  import type { IconComponent } from '$lib/components/icon';
  import Logo from '$lib/components/Logo.svelte';
  import { motion } from '$lib/stores/motion.svelte';

  const POINTS: { icon: IconComponent; title: string; body: string }[] = [
    { icon: Eye, title: 'See your setup', body: 'Read your OMP and PI configuration and extensions in one place, secrets masked.' },
    { icon: HardDrive, title: 'Your harness stays yours', body: 'Harness Desktop uses the CLIs you already installed. Nothing is copied or uploaded.' },
    { icon: ShieldCheck, title: 'Local and private', body: 'No telemetry, no accounts. The only network use is checking for updates, and links you click.' },
  ];
</script>

<div class="flex flex-col items-start gap-8">
  <Logo size={72} wordmark animate={motion.level === 'full'} class="text-fg" />

  <header class="flex flex-col gap-2">
    <h2 id="onboarding-title" tabindex="-1" use:autofocus class="text-4xl text-fg outline-none">Welcome. Let’s find your harness.</h2>
    <p class="max-w-xl text-lg text-fg-2">A one-minute setup: detect your agent CLIs, choose your daily driver, and open a project.</p>
  </header>

  <ul class="grid w-full grid-cols-3 gap-4">
    {#each POINTS as point (point.title)}
      <li class="flex flex-col gap-2 rounded-lg border border-line bg-glass p-4 backdrop-blur-md">
        <point.icon class="text-accent-text" size={20} strokeWidth={1.8} aria-hidden="true" />
        <p class="text-base font-medium text-fg">{point.title}</p>
        <p class="text-sm text-fg-2">{point.body}</p>
      </li>
    {/each}
  </ul>
</div>
