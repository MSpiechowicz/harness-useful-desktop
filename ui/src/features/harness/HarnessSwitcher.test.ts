import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import HarnessSwitcher from './HarnessSwitcher.svelte';

/** The fixture harnesses, with `missing` ones reported as not installed. */
function listed(missing: string[] = []) {
  return fixtures.harnesses.map((summary) =>
    missing.includes(summary.info.id) ? { ...summary, detection: { ...summary.detection, installed: false } } : summary,
  );
}

beforeEach(() => {
  prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'omp' });
  harness.summaries = listed();
});

afterEach(() => {
  vi.restoreAllMocks();
});

async function openMenu() {
  render(HarnessSwitcher);
  await fireEvent.click(screen.getByRole('button', { name: /^Switch harness/ }));

  return screen.findByRole('menu');
}

describe('harness switcher', () => {
  it('is absent when there is nothing to switch to', () => {
    harness.summaries = listed().slice(0, 1);

    render(HarnessSwitcher);

    expect(screen.queryByRole('button', { name: /^Switch harness/ })).not.toBeInTheDocument();
  });

  it('names the active harness on its button and lists every detected harness with the active one checked', async () => {
    render(HarnessSwitcher);
    const trigger = screen.getByRole('button', { name: /^Switch harness/ });

    expect(trigger).toHaveTextContent(harness.active!.info.shortName);

    await fireEvent.click(trigger);
    const menu = await screen.findByRole('menu');

    expect(within(menu).getAllByRole('menuitemradio')).toHaveLength(harness.installed.length);
    expect(within(menu).getByRole('menuitemradio', { checked: true })).toHaveTextContent(harness.active!.info.name);
  });

  it('leaves out a harness that is not detected', async () => {
    harness.summaries = listed(['pi']);
    const menu = await openMenu();
    const names = within(menu).getAllByRole('menuitemradio').map((row) => row.textContent?.trim());

    expect(harness.installed.map((summary) => summary.info.id)).not.toContain('pi');
    expect(names).toHaveLength(harness.installed.length);
    harness.installed.forEach((summary, index) => expect(names[index]).toMatch(new RegExp(`${summary.info.name}$`)));
  });

  it('is absent when only one harness is detected', () => {
    harness.summaries = listed(fixtures.harnesses.map((summary) => summary.info.id).slice(1));

    render(HarnessSwitcher);

    expect(screen.queryByRole('button', { name: /^Switch harness/ })).not.toBeInTheDocument();
  });

  it('switches to the chosen harness and closes the menu', async () => {
    const select = vi.spyOn(harness, 'select').mockResolvedValue();
    const menu = await openMenu();
    const other = harness.summaries.find((summary) => summary.info.id !== harness.active!.info.id && summary.detection.installed)!;

    await fireEvent.click(within(menu).getByRole('menuitemradio', { name: new RegExp(other.info.name) }));

    expect(select).toHaveBeenCalledWith(other.info.id);
    await waitFor(() => expect(screen.queryByRole('menu')).not.toBeInTheDocument());
  });
});
