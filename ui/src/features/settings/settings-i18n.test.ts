import { render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { i18n } from '$lib/i18n/i18n.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { updates } from '$lib/stores/updates.svelte';

import { samplePrefs } from '../../../tests/helpers';
import { formatBytes } from '../updates/copy';
import RestartConfirm from '../updates/RestartConfirm.svelte';
import { SETTINGS_SECTIONS } from './sections';
import SettingsView from './SettingsView.svelte';

beforeAll(() => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off' });
  Element.prototype.scrollIntoView = vi.fn();
});

afterEach(() => {
  updates.confirmingRestart = false;
  updates.chatsAtRisk = 0;
});

describe('Settings in Polish', () => {
  it('translates the section nav, the rows and the palette words, and offers a language row', async () => {
    await i18n.load('pl');
    render(SettingsView);

    const nav = screen.getByRole('navigation', { name: 'Sekcje ustawień' });

    expect(within(nav).getAllByRole('button').map((button) => button.textContent?.trim())).toEqual(SETTINGS_SECTIONS.map((section) => section.label));
    expect(within(nav).getByRole('button', { name: 'Prywatność i bezpieczeństwo' })).toBeInTheDocument();
    expect(SETTINGS_SECTIONS[0].keywords).toContain('język');

    const appearance = screen.getByRole('region', { name: 'Wygląd' });

    expect(within(appearance).getByText('Motyw')).toBeInTheDocument();
    expect(within(appearance).getByRole('button', { name: 'Język: Polski' })).toBeInTheDocument();
    expect(within(appearance).getByRole('radio', { name: 'Ciemny' })).toBeInTheDocument();
  });

  it('gives the privacy facts in Polish and keeps the stored-preferences fact complete', async () => {
    await i18n.load('pl');
    render(SettingsView);

    const stored = screen.getByText('Zapisane na tym urządzeniu').closest('li')!;

    expect(stored).toHaveTextContent('motyw, język, animacje');
    expect(screen.getByText(/Nie zapisujemy treści czatów, tokenów ani haseł/)).toBeInTheDocument();
  });

  it.each([
    [1, '1 działający czat zostanie zatrzymany'],
    [2, '2 działające czaty zostaną zatrzymane'],
    [5, '5 działających czatów zostanie zatrzymanych'],
    [22, '22 działające czaty zostaną zatrzymane'],
  ])('warns about %i running chats with the right Polish plural', async (chats, expected) => {
    await i18n.load('pl');
    updates.chatsAtRisk = chats;
    updates.confirmingRestart = true;
    render(RestartConfirm);

    expect(await screen.findByRole('dialog', { name: 'Uruchomić ponownie teraz?' })).toHaveTextContent(expected);
  });

  it('writes download sizes with the decimal comma of the language', async () => {
    expect(formatBytes(1_572_864)).toBe('1.5 MB');

    await i18n.load('pl');

    expect(formatBytes(1_572_864)).toBe('1,5 MB');

    await i18n.load('fr');

    expect(formatBytes(1_572_864)).toBe('1,5 Mo');
  });
});
