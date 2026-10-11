<script lang="ts">
  import Monitor from '@lucide/svelte/icons/monitor';
  import Moon from '@lucide/svelte/icons/moon';
  import Play from '@lucide/svelte/icons/play';
  import Settings from '@lucide/svelte/icons/settings';
  import Sun from '@lucide/svelte/icons/sun';
  import Wand from '@lucide/svelte/icons/wand-sparkles';
  import { tick } from 'svelte';

  import { scrollMemory } from '$lib/a11y/scroll-memory';
  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import Switch from '$lib/components/Switch.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import type { MotionPref } from '$lib/ipc/bindings/MotionPref';
  import type { ThemePref } from '$lib/ipc/bindings/ThemePref';
  import { motion } from '$lib/stores/motion.svelte';
  import { prefs } from '$lib/stores/prefs.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { viewState, type SettingsSectionId } from '$lib/stores/view-state.svelte';
  import { cx } from '$lib/utils/cx';

  import { intro } from '../intro/intro-state.svelte';
  import { onboarding } from '../onboarding/onboarding-state.svelte';
  import AboutCard from './AboutCard.svelte';
  import HarnessSettings from './HarnessSettings.svelte';
  import PrivacySection from './PrivacySection.svelte';
  import SettingRow from './SettingRow.svelte';
  import { activeSection, SETTINGS_SECTIONS, type SectionBox } from './sections';
  import SettingsSection from './SettingsSection.svelte';
  import UpdatesSection from './UpdatesSection.svelte';

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

  /** The reading line, in px below the top of the scroller, that decides which section is current. */
  const READING_LINE_PX = 120;

  let scroller: HTMLElement | undefined = $state();
  let current = $state<SettingsSectionId>('appearance');

  function sectionElement(id: SettingsSectionId): HTMLElement | null {
    return scroller?.querySelector<HTMLElement>(`[data-settings-section="${id}"]`) ?? null;
  }

  function spy(): void {
    if (!scroller) {
      return;
    }

    const origin = scroller.getBoundingClientRect().top;
    const boxes = SETTINGS_SECTIONS.flatMap<SectionBox>(({ id }) => {
      const element = sectionElement(id);

      return element ? [{ id, top: element.getBoundingClientRect().top - origin }] : [];
    });
    const atEnd = scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 2;

    current = activeSection(boxes, READING_LINE_PX, atEnd) ?? current;
  }

  function jump(id: SettingsSectionId): void {
    current = id;
    sectionElement(id)?.scrollIntoView({ behavior: motion.level === 'full' ? 'smooth' : 'auto', block: 'start' });
  }

  // The command palette can ask for a section; scroll to it once it has been laid out.
  $effect(() => {
    const target = viewState.settingsTarget;
    if (!target || !prefs.value) {
      return;
    }

    viewState.settingsTarget = null;
    void tick().then(() => requestAnimationFrame(() => jump(target)));
  });
</script>

<div class="@container flex size-full min-h-0 flex-col">
  <ViewHeader title="Settings" icon={Settings} context="Changes apply instantly and are saved on this device" />

  <div bind:this={scroller} use:scrollMemory={'settings'} onscroll={spy} class="min-h-0 flex-1 overflow-y-auto">
    <div class="mx-auto w-full max-w-[1152px] px-[var(--gutter)] py-6 @4xl:grid @4xl:grid-cols-[208px_minmax(0,1fr)] @4xl:gap-10 @4xl:py-8">
      <!-- Section nav: a sticky chip row on narrow panes, a left rail with scroll-spy on wide ones. -->
      <nav
        aria-label="Settings sections"
        class="sticky top-0 z-10 -mx-[var(--gutter)] mb-6 flex gap-2 overflow-x-auto bg-canvas/80 px-[var(--gutter)] py-2 backdrop-blur-xl @4xl:top-8 @4xl:z-0 @4xl:m-0 @4xl:mb-0 @4xl:flex-col @4xl:gap-1 @4xl:self-start @4xl:overflow-visible @4xl:bg-transparent @4xl:p-0 @4xl:backdrop-blur-none"
      >
        {#each SETTINGS_SECTIONS as section (section.id)}
          {@const selected = section.id === current}
          <button
            type="button"
            aria-current={selected ? 'true' : undefined}
            onclick={() => jump(section.id)}
            class={cx(
              'inline-flex h-9 shrink-0 items-center gap-2.5 rounded-md border px-3 text-sm font-medium whitespace-nowrap transition-colors duration-[var(--dur-fast)] active:scale-[0.98]',
              selected ? 'border-accent/30 bg-accent-wash text-fg' : 'border-transparent text-fg-2 hover:bg-hover active:bg-press hover:text-fg',
            )}
          >
            <section.icon size={ICON.sm} strokeWidth={1.8} class={selected ? 'text-accent-text' : ''} aria-hidden="true" />
            {section.label}
          </button>
        {/each}
      </nav>

      <div class="mx-auto flex w-full max-w-3xl flex-col gap-8 @4xl:mx-0 @4xl:max-w-none">
        {#if prefs.value}
          <SettingsSection id="appearance" title="Appearance">
            <SettingRow title="Theme" description="Follow your system or pick one. The change previews immediately.">
              {#snippet control()}
                <SegmentedControl size="md" label="Theme" options={THEME_OPTIONS} bind:value={() => prefs.value?.theme ?? 'system', (theme) => void prefs.update({ theme })} />
              {/snippet}
            </SettingRow>

            <SettingRow title="Show thought process" description="The agent's reasoning, folded under each reply. Also toggled from the chat header.">
              {#snippet control()}
                <Switch label="Show thought process" bind:checked={() => prefs.value?.showThinking ?? true, (showThinking) => void prefs.update({ showThinking })} />
              {/snippet}
            </SettingRow>

            <SettingRow title="Motion" description="Reduced keeps simple fades; Off disables all animation. Your OS reduced-motion setting always applies.">
              {#snippet control()}
                <SegmentedControl size="md" label="Motion" options={MOTION_OPTIONS} bind:value={() => prefs.value?.motion ?? 'full', (next) => void prefs.update({ motion: next })} />
              {/snippet}
            </SettingRow>
          </SettingsSection>

          <SettingsSection id="startup" title="Startup">
            <SettingRow title="Play intro on launch" description="A short animated welcome when the app opens. Any key or click skips it.">
              {#snippet control()}
                <Switch label="Play intro on launch" bind:checked={() => prefs.value?.introEnabled ?? true, (introEnabled) => void prefs.update({ introEnabled })} />
              {/snippet}
            </SettingRow>

            <SettingRow title="Intro sound" description="The logo's sound while the intro plays. It is never played when the intro is off or skipped.">
              {#snippet control()}
                <Switch label="Intro sound" bind:checked={() => prefs.value?.soundEnabled ?? false, (soundEnabled) => void prefs.update({ soundEnabled })} />
              {/snippet}
            </SettingRow>

            <SettingRow title="Replay intro" description={motion.level === 'off' ? 'Motion is off; turn it on to replay the intro.' : 'Watch it once more.'}>
              {#snippet control()}
                <Button disabled={motion.level === 'off'} onclick={() => intro.play(motion.level)}>
                  {#snippet leading()}<Play size={ICON.sm} aria-hidden="true" />{/snippet}
                  Replay intro
                </Button>
              {/snippet}
            </SettingRow>
          </SettingsSection>

          <SettingsSection id="harnesses" title="Harnesses">
            <HarnessSettings />

            <SettingRow title="Setup" description="Detect your harnesses again, choose your daily driver and a project.">
              {#snippet control()}
                <Button onclick={() => onboarding.restart()}>
                  {#snippet leading()}<Wand size={ICON.sm} aria-hidden="true" />{/snippet}
                  Run onboarding again
                </Button>
              {/snippet}
            </SettingRow>
          </SettingsSection>

          <SettingsSection id="projects" title="Projects">
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

          <UpdatesSection id="updates" />

          <PrivacySection id="privacy" />
        {/if}

        <div id="about" data-settings-section="about" class="scroll-mt-6"><AboutCard /></div>
      </div>
    </div>
  </div>
</div>
