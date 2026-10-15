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
  import SearchField from '$lib/components/SearchField.svelte';
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import type { StripOption } from '$lib/components/strip';
  import Switch from '$lib/components/Switch.svelte';
  import LangPicker from '$lib/components/LangPicker.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
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

  // Labels are getters so they follow the language on screen.
  const THEME_OPTIONS: StripOption<ThemePref>[] = [
    {
      value: 'system',
      get label() {
        return t('settings.theme.system');
      },
      icon: Monitor,
    },
    {
      value: 'dark',
      get label() {
        return t('settings.theme.dark');
      },
      icon: Moon,
    },
    {
      value: 'light',
      get label() {
        return t('settings.theme.light');
      },
      icon: Sun,
    },
  ];
  const MOTION_OPTIONS: StripOption<MotionPref>[] = [
    {
      value: 'full',
      get label() {
        return t('settings.motion.full');
      },
    },
    {
      value: 'reduced',
      get label() {
        return t('settings.motion.reduced');
      },
    },
    {
      value: 'off',
      get label() {
        return t('settings.motion.off');
      },
    },
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

  let query = $state('');
  /** Ids of the sections whose text contains the query; null while the query is empty. */
  let matches = $state<Set<string> | null>(null);
  const visibleSections = $derived(matches ? SETTINGS_SECTIONS.filter((section) => matches!.has(section.id)) : SETTINGS_SECTIONS);

  // Sections own opaque children (updates, privacy, about), so match on their rendered text and hide the rest.
  $effect(() => {
    const needle = query.trim().toLocaleLowerCase();
    const loaded = prefs.value;
    void t('settings.title'); // re-run when the language changes

    void tick().then(() => {
      if (!scroller || !loaded) {
        return;
      }

      const hits = new Set<string>();

      for (const { id } of SETTINGS_SECTIONS) {
        const element = sectionElement(id);

        if (!element) {
          continue;
        }

        const hit = needle === '' || (element.textContent ?? '').toLocaleLowerCase().includes(needle);

        element.style.display = hit ? '' : 'none';

        if (hit) {
          hits.add(id);
        }
      }

      matches = needle === '' ? null : hits;
    });
  });
</script>

<div class="@container flex size-full min-h-0 flex-col">
  <ViewHeader title={t('settings.title')} icon={Settings} context={t('settings.context')}>
    {#snippet actions()}
      <div class="w-[340px] max-[900px]:w-[220px]">
        <SearchField bind:value={query} label={t('settings.search.label')} placeholder={t('settings.search.placeholder')} shortcut="mod+f" />
      </div>
    {/snippet}
  </ViewHeader>

  <div bind:this={scroller} use:scrollMemory={'settings'} onscroll={spy} class="min-h-0 flex-1 overflow-y-auto">
    <div class="mx-auto w-full max-w-[1152px] px-[var(--gutter)] py-6 @4xl:grid @4xl:grid-cols-[208px_minmax(0,1fr)] @4xl:gap-10 @4xl:py-8">
      <!-- Section nav: a sticky chip row on narrow panes, a left rail with scroll-spy on wide ones. -->
      <nav
        aria-label={t('settings.nav.label')}
        class="sticky top-0 z-10 -mx-[var(--gutter)] mb-6 flex gap-2 overflow-x-auto bg-canvas/80 px-[var(--gutter)] py-2 backdrop-blur-xl @4xl:top-8 @4xl:z-0 @4xl:m-0 @4xl:mb-0 @4xl:flex-col @4xl:gap-1 @4xl:self-start @4xl:overflow-visible @4xl:bg-transparent @4xl:p-0 @4xl:backdrop-blur-none"
      >
        {#each visibleSections as section (section.id)}
          {@const selected = section.id === current}
          <button
            type="button"
            aria-current={selected ? 'true' : undefined}
            onclick={() => jump(section.id)}
            class={cx(
              'inline-flex h-control max-w-full shrink-0 items-center gap-2.5 rounded-md border px-3 text-sm font-medium whitespace-nowrap transition-colors duration-[var(--dur-fast)] active:scale-[0.98]',
              selected ? 'border-accent/30 bg-accent-wash text-fg' : 'border-transparent text-fg-2 hover:bg-hover active:bg-press hover:text-fg',
            )}
          >
            <!-- Long labels in other languages ("Prywatność i bezpieczeństwo") truncate; the icon never shrinks. -->
            <section.icon size={ICON.sm} strokeWidth={1.8} class={cx('shrink-0', selected && 'text-accent-text')} aria-hidden="true" />
            <span class="min-w-0 truncate" title={section.label}>{section.label}</span>
          </button>
        {/each}
      </nav>

      <div class="mx-auto flex w-full max-w-3xl flex-col gap-8 @4xl:mx-0 @4xl:max-w-none">
        {#if prefs.value}
          <SettingsSection id="appearance" title={t('settings.section.appearance')}>
            <SettingRow title={t('settings.theme.title')} description={t('settings.theme.description')}>
              {#snippet control()}
                <SegmentedControl label={t('settings.theme.title')} options={THEME_OPTIONS} bind:value={() => prefs.value?.theme ?? 'system', (theme) => void prefs.update({ theme })} />
              {/snippet}
            </SettingRow>

            <SettingRow title={t('settings.language.title')} description={t('settings.language.description')}>
              {#snippet control()}
                <LangPicker wide />
              {/snippet}
            </SettingRow>

            <SettingRow title={t('settings.thinking.title')} description={t('settings.thinking.description')}>
              {#snippet control()}
                <Switch label={t('settings.thinking.title')} bind:checked={() => prefs.value?.showThinking ?? true, (showThinking) => void prefs.update({ showThinking })} />
              {/snippet}
            </SettingRow>

            <SettingRow title={t('settings.usage.title')} description={t('settings.usage.description')}>
              {#snippet control()}
                <Switch label={t('settings.usage.title')} bind:checked={() => prefs.value?.showMessageUsage ?? true, (showMessageUsage) => void prefs.update({ showMessageUsage })} />
              {/snippet}
            </SettingRow>

            <SettingRow title={t('settings.aiNames.title')} description={t('settings.aiNames.description')} points={[t('settings.aiNames.cost'), t('settings.aiNames.sent'), t('settings.aiNames.off')]}>
              {#snippet control()}
                <Switch label={t('settings.aiNames.title')} bind:checked={() => prefs.value?.aiChatNames ?? false, (aiChatNames) => void prefs.update({ aiChatNames })} />
              {/snippet}
            </SettingRow>

            <SettingRow title={t('settings.motion.title')} description={t('settings.motion.description')}>
              {#snippet control()}
                <SegmentedControl label={t('settings.motion.title')} options={MOTION_OPTIONS} bind:value={() => prefs.value?.motion ?? 'full', (next) => void prefs.update({ motion: next })} />
              {/snippet}
            </SettingRow>
          </SettingsSection>

          <SettingsSection id="startup" title={t('settings.section.startup')}>
            <SettingRow title={t('settings.intro.title')} description={t('settings.intro.description')}>
              {#snippet control()}
                <Switch label={t('settings.intro.title')} bind:checked={() => prefs.value?.introEnabled ?? true, (introEnabled) => void prefs.update({ introEnabled })} />
              {/snippet}
            </SettingRow>

            <SettingRow title={t('settings.sound.title')} description={t('settings.sound.description')}>
              {#snippet control()}
                <Switch label={t('settings.sound.title')} bind:checked={() => prefs.value?.soundEnabled ?? false, (soundEnabled) => void prefs.update({ soundEnabled })} />
              {/snippet}
            </SettingRow>

            <SettingRow title={t('settings.replay.title')} description={motion.level === 'off' ? t('settings.replay.motionOff') : t('settings.replay.ready')}>
              {#snippet control()}
                <Button disabled={motion.level === 'off'} onclick={() => intro.play(motion.level)}>
                  {#snippet leading()}<Play size={ICON.sm} aria-hidden="true" />{/snippet}
                  {t('settings.replay.title')}
                </Button>
              {/snippet}
            </SettingRow>
          </SettingsSection>

          <SettingsSection id="harnesses" title={t('settings.section.harnesses')}>
            <HarnessSettings />

            <SettingRow title={t('settings.setup.title')} description={t('settings.setup.description')}>
              {#snippet control()}
                <Button onclick={() => onboarding.restart()}>
                  {#snippet leading()}<Wand size={ICON.sm} aria-hidden="true" />{/snippet}
                  {t('settings.setup.run')}
                </Button>
              {/snippet}
            </SettingRow>
          </SettingsSection>

          <SettingsSection id="projects" title={t('settings.section.projects')}>
            <SettingRow title={t('settings.recents.title')} description={t('settings.recents.description')}>
              {#snippet control()}
                <Switch
                  label={t('settings.recents.title')}
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
        {#if matches?.size === 0}
          <p class="px-1 text-sm text-fg-3" role="status">{t('settings.search.noMatch')}</p>
        {/if}
      </div>
    </div>
  </div>
</div>
