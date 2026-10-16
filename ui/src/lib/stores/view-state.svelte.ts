import type { ViewId } from './nav.svelte';

/** The settings sections the palette and the section nav can jump to. */
export type SettingsSectionId = 'appearance' | 'startup' | 'harnesses' | 'memory' | 'projects' | 'updates' | 'privacy' | 'sync' | 'about';

/**
 * What each view had selected when the user left it, so coming back (or jumping in from the
 * command palette) lands where they were. Views are unmounted on every switch; this outlives them.
 */
export class ViewState {
  configSection = $state<string | null>(null);
  /** Filter of the open configuration section; the palette sets it to highlight a key. */
  configQuery = $state('');

  /** The kind of the selected extension (an id alone is not unique across kinds). */
  extensionsTab = $state<string | null>(null);
  extensionsItem = $state<string | null>(null);
  extensionsQuery = $state('');

  sessionId = $state<string | null>(null);
  /** What the Sessions search field in the top bar holds. */
  sessionsQuery = $state('');

  /** A settings section waiting to be scrolled to; the settings view clears it once it has. */
  settingsTarget = $state<SettingsSectionId | null>(null);

  private scrolls = new Map<string, number>();

  scrollOf(key: string): number {
    return this.scrolls.get(key) ?? 0;
  }

  rememberScroll(key: string, top: number): void {
    this.scrolls.set(key, top);
  }

  /** Forgets all remembered state: a fresh start. */
  resetAll(): void {
    this.reset('config');
    this.reset('extensions');
    this.reset('sessions');
    this.settingsTarget = null;
    this.scrolls.clear();
  }

  /** Forgets everything for one view (e.g. after switching harness, when ids no longer apply). */
  reset(view: ViewId): void {
    if (view === 'config') {
      this.configSection = null;
      this.configQuery = '';
    } else if (view === 'extensions') {
      this.extensionsTab = null;
      this.extensionsItem = null;
      this.extensionsQuery = '';
    } else if (view === 'sessions') {
      this.sessionId = null;
      this.sessionsQuery = '';
    }
  }
}

export const viewState = new ViewState();
