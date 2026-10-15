<script lang="ts">
  import Eye from '@lucide/svelte/icons/eye';
  import HardDrive from '@lucide/svelte/icons/hard-drive';
  import ShieldCheck from '@lucide/svelte/icons/shield-check';

  import { autofocus } from '$lib/a11y/autofocus';
  import type { IconComponent } from '$lib/components/icon';
  import Logo from '$lib/components/Logo.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  const POINTS = $derived<{ icon: IconComponent; title: string; body: string }[]>([
    { icon: Eye, title: t('onboarding.welcome.see.title'), body: t('onboarding.welcome.see.body') },
    { icon: HardDrive, title: t('onboarding.welcome.yours.title'), body: t('onboarding.welcome.yours.body') },
    { icon: ShieldCheck, title: t('onboarding.welcome.private.title'), body: t('onboarding.welcome.private.body') },
  ]);
</script>

<!-- The brand is a quiet signature with room of its own; the headline is the hero of this step. -->
<div class="flex flex-col items-center gap-10 text-center [@media(max-height:720px)]:gap-6">
  <Logo size={28} wordmark class="mb-6 [@media(max-height:720px)]:mb-2" />

  <header class="flex flex-col items-center gap-4">
    <h2
      id="onboarding-title"
      tabindex="-1"
      use:autofocus
      class="max-w-[22ch] text-[clamp(2.25rem,1.3rem+3vw,3.75rem)] leading-[1.05] tracking-tight text-balance text-fg outline-none"
    >
      {t('onboarding.welcome.lead')} <span class="text-accent-text">{t('onboarding.welcome.accent')}</span>
    </h2>
    <p class="max-w-xl text-lg text-pretty text-fg-2">{t('onboarding.welcome.body')}</p>
  </header>

  <ul class="grid w-full grid-cols-1 gap-4 text-left sm:grid-cols-3">
    {#each POINTS as point (point.title)}
      <li
        class="flex flex-col gap-3 rounded-lg border-card border-line bg-glass p-5 transition-[border-color,box-shadow] duration-[var(--dur-base)] hover:edge-accent hover:ring-1 hover:ring-accent/30"
      >
        <span class="grid size-10 place-items-center rounded-md bg-accent-wash">
          <point.icon class="text-accent-text" size={20} strokeWidth={1.8} aria-hidden="true" />
        </span>
        <p class="text-base font-medium text-fg">{point.title}</p>
        <p class="text-sm text-fg-2">{point.body}</p>
      </li>
    {/each}
  </ul>
</div>

