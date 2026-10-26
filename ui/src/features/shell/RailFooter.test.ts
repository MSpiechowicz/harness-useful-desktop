import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { i18n } from '$lib/i18n/i18n.svelte';
import { ipc } from '$lib/ipc/client';
import { app } from '$lib/stores/app.svelte';
import { ui } from '$lib/stores/ui-state.svelte';
import { language } from '$lib/stores/language.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import Rail from './Rail.svelte';
import RailFooter from './RailFooter.svelte';

let stopLanguage = (): void => {};

beforeAll(() => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
});

afterEach(() => {
  stopLanguage();
  stopLanguage = (): void => {};
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  void i18n.load('en');
  ui.setRailExpanded(false);
  prefs.value = samplePrefs({ motion: 'off' });
});

function ready(): void {
  app.info = { name: 'Harness Desktop', version: '0.2.0', os: 'linux', arch: 'x86_64', debug: false };
  app.status = 'ready';
}

describe('rail footer, expanded', () => {
  it('offers the language, the theme choices and the version', () => {
    ready();
    render(RailFooter, { expanded: true });

    expect(screen.getByRole('button', { name: 'Language: English' })).toHaveTextContent('English');
    const theme = screen.getByRole('radiogroup', { name: 'Theme' });
    expect(within(theme).getAllByRole('radio').map((radio) => radio.getAttribute('aria-label'))).toEqual(['System', 'Light', 'Dark']);
    expect(screen.getByText('v0.2.0')).toBeInTheDocument();
    expect(screen.getByText('Ready')).toBeInTheDocument();
  });

  it('saves the chosen theme', async () => {
    ready();
    const invoke = vi.spyOn(ipc, 'invoke');
    render(RailFooter, { expanded: true });

    await fireEvent.click(screen.getByRole('radio', { name: 'Dark' }));

    expect(prefs.value?.theme).toBe('dark');
    expect(invoke).toHaveBeenCalledWith('prefs_set', { prefs: expect.objectContaining({ theme: 'dark' }) });
  });

  it('lists the languages by their own names with a check on the current one', async () => {
    ready();
    render(RailFooter, { expanded: true });

    await fireEvent.click(screen.getByRole('button', { name: 'Language: English' }));

    const options = await screen.findAllByRole('menuitemradio');
    expect(options.map((option) => option.textContent?.trim())).toEqual(['English', 'Deutsch', 'Español', 'Français', 'Polski', 'Português (Brasil)']);
    expect(options.map((option) => option.getAttribute('aria-checked'))).toEqual(['true', 'false', 'false', 'false', 'false', 'false']);
  });

  it('picks English while the saved Polish is still loading', async () => {
    ready();
    prefs.value = samplePrefs({ motion: 'off', language: 'pl' });
    const invoke = vi.spyOn(ipc, 'invoke');
    render(RailFooter, { expanded: true });

    expect(i18n.lang).toBe('en');
    await fireEvent.click(screen.getByRole('button', { name: 'Language: English' }));
    await fireEvent.click(await screen.findByRole('menuitemradio', { name: 'English' }));

    expect(invoke).toHaveBeenCalledWith('prefs_set', { prefs: expect.objectContaining({ language: 'en' }) });
  });

  it('does not save the language that is already saved', async () => {
    ready();
    prefs.value = samplePrefs({ motion: 'off', language: 'pl' });
    const invoke = vi.spyOn(ipc, 'invoke');
    render(RailFooter, { expanded: true });

    await fireEvent.click(screen.getByRole('button', { name: 'Language: English' }));
    await fireEvent.click(await screen.findByRole('menuitemradio', { name: 'Polski' }));

    expect(invoke).not.toHaveBeenCalledWith('prefs_set', expect.anything());
  });

  it('saves the picked language and turns the rail Polish', async () => {
    ready();
    stopLanguage = language.start();
    const invoke = vi.spyOn(ipc, 'invoke');
    vi.stubGlobal('matchMedia', (query: string) => ({ matches: query.includes('min-width'), media: query, addEventListener: () => {}, removeEventListener: () => {} }));
    ui.setRailExpanded(true);
    render(Rail);
    const rail = screen.getByRole('navigation', { name: 'Primary' });

    await fireEvent.click(within(rail).getByRole('button', { name: 'Language: English' }));
    await fireEvent.click(await screen.findByRole('menuitemradio', { name: 'Polski' }));

    expect(invoke).toHaveBeenCalledWith('prefs_set', { prefs: expect.objectContaining({ language: 'pl' }) });
    await waitFor(() => expect(screen.getByRole('navigation', { name: 'Menu główne' })).toBeInTheDocument());
    expect(screen.getByRole('button', { name: 'Język: Polski' })).toHaveTextContent('Polski');
    expect(document.documentElement.lang).toBe('pl');
    expect(i18n.lang).toBe('pl');
  });

  it('saves Brazilian Portuguese and turns the rail Portuguese', async () => {
    ready();
    stopLanguage = language.start();
    const invoke = vi.spyOn(ipc, 'invoke');
    vi.stubGlobal('matchMedia', (query: string) => ({ matches: query.includes('min-width'), media: query, addEventListener: () => {}, removeEventListener: () => {} }));
    ui.setRailExpanded(true);
    render(Rail);
    const rail = screen.getByRole('navigation', { name: 'Primary' });

    await fireEvent.click(within(rail).getByRole('button', { name: 'Language: English' }));
    await fireEvent.click(await screen.findByRole('menuitemradio', { name: 'Português (Brasil)' }));

    expect(invoke).toHaveBeenCalledWith('prefs_set', { prefs: expect.objectContaining({ language: 'pt' }) });
    await waitFor(() => expect(screen.getByRole('navigation', { name: 'Principal' })).toBeInTheDocument());
    expect(screen.getByRole('button', { name: 'Idioma: Português (Brasil)' })).toHaveTextContent('Português (Brasil)');
    expect(i18n.lang).toBe('pt');
  });
});

describe('rail footer, collapsed', () => {
  it('shows only the status dot, with the status and version as its title', () => {
    ready();
    const { container } = render(RailFooter, { expanded: false });

    expect(screen.queryByRole('button', { name: 'Language: English' })).toBeNull();
    expect(screen.queryByRole('radiogroup')).toBeNull();
    expect(screen.queryByText('v0.2.0')).toBeNull();
    expect(container.querySelector('[title="Ready · v0.2.0"]')).not.toBeNull();
  });

  it('adds no live region of its own', () => {
    ready();
    render(RailFooter, { expanded: true });

    expect(screen.queryByRole('status')).toBeNull();
  });
});
