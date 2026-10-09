<script lang="ts">
  import Monitor from '@lucide/svelte/icons/monitor';
  import Moon from '@lucide/svelte/icons/moon';
  import Play from '@lucide/svelte/icons/play';
  import Wand from '@lucide/svelte/icons/wand-sparkles';
  import Sun from '@lucide/svelte/icons/sun';

  import Button from '$lib/components/Button.svelte';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import Switch from '$lib/components/Switch.svelte';
  import type { MotionPref } from '$lib/ipc/bindings/MotionPref';
  import type { ThemePref } from '$lib/ipc/bindings/ThemePref';
  import { motion } from '$lib/stores/motion.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';

  import { project } from '$lib/stores/project.svelte';

  import { intro } from '../intro/intro.svelte';
  import { onboarding } from '../onboarding/onboarding.svelte';
  import HarnessSettings from './HarnessSettings.svelte';
  import AboutCard from './AboutCard.svelte';
  import SettingRow from './SettingRow.svelte';
  import SettingsSection from './SettingsSection.svelte';

  const THEME_OPTIONS: StripOption<ThemePref>[] = [
    { value: 'system', label: 'System', icon: Monitor },
    { value: 'dark', label: 'Dark', icon: Moon },
    { value: 'light', label: 'Light', icon: Sun },
  ];
  const MOTION_OPTIONS: StripOption<MotionPref>[] = [
    { value: 'full', label: 'Full' },
    { value: 'reduced', label: 'Reduced' },
    { value: 'off', label: 'Off' },
  ];
</script>

<div class="mx-auto flex w-full max-w-3xl flex-col gap-8 px-8 py-10">
  <header class="flex flex-col gap-1">
    <h2 class="text-3xl text-fg">Settings</h2>
    <p class="text-base text-fg-2">Changes apply instantly and are saved on this device.</p>
  </header>

  {#if prefs.value}
    <SettingsSection title="Appearance">
      <SettingRow title="Theme" description="Follow your system or pick one. The change previews immediately.">
        {#snippet control()}
          <SegmentedControl
            label="Theme"
            options={THEME_OPTIONS}
            bind:value={() => prefs.value?.theme ?? 'system', (theme) => void prefs.update({ theme })}
          />
        {/snippet}
      </SettingRow>

      <SettingRow title="Motion" description="Reduced keeps simple fades; Off disables all animation. Your OS reduced-motion setting always applies.">
        {#snippet control()}
          <SegmentedControl
            label="Motion"
            options={MOTION_OPTIONS}
            bind:value={() => prefs.value?.motion ?? 'full', (next) => void prefs.update({ motion: next })}
          />
        {/snippet}
      </SettingRow>
    </SettingsSection>

    <SettingsSection title="Startup">
      <SettingRow title="Play intro on launch" description="A short animated welcome when the app opens. Any key or click skips it.">
        {#snippet control()}
          <Switch
            label="Play intro on launch"
            bind:checked={() => prefs.value?.introEnabled ?? true, (introEnabled) => void prefs.update({ introEnabled })}
          />
        {/snippet}
      </SettingRow>

      <SettingRow title="Replay intro" description={motion.level === 'off' ? 'Motion is off; turn it on to replay the intro.' : 'Watch it once more.'}>
        {#snippet control()}
          <Button disabled={motion.level === 'off'} onclick={() => intro.play(motion.level)}>
            {#snippet leading()}<Play size={14} aria-hidden="true" />{/snippet}
            Replay intro
          </Button>
        {/snippet}
      </SettingRow>
    </SettingsSection>

    <SettingsSection title="Harnesses">
      <HarnessSettings />

      <SettingRow title="Setup" description="Detect your harnesses again, choose your daily driver and a project.">
        {#snippet control()}
          <Button onclick={() => onboarding.restart()}>
            {#snippet leading()}<Wand size={14} aria-hidden="true" />{/snippet}
            Run onboarding again
          </Button>
        {/snippet}
      </SettingRow>
    </SettingsSection>

    <SettingsSection title="Projects">
      <SettingRow title="Remember recent projects" description="Keep a list of the folders you opened. Turning this off clears the list.">
        {#snippet control()}
          <Switch
            label="Remember recent projects"
            bind:checked={
              () => prefs.value?.rememberRecentProjects ?? true,
              (rememberRecentProjects) => void prefs.update({ rememberRecentProjects }).then(() => project.refreshRecents())
            }
          />
        {/snippet}
      </SettingRow>
    </SettingsSection>
  {/if}

  <AboutCard />
</div>
