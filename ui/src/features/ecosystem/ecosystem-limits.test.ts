import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import type { LimitGaugePref } from '$lib/ipc/bindings/LimitGaugePref';
import { prefs } from '$lib/stores/prefs.svelte';

import { freshBackend } from '../../../tests/ecosystem-backend';
import { samplePrefs } from '../../../tests/helpers';
import EcosystemView from './EcosystemView.svelte';

beforeAll(() => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
});

afterEach(async () => {
  // A save still on its way would put its answer over the next test's preferences.
  await new Promise((resolve) => setTimeout(resolve, 10));
  vi.restoreAllMocks();
});

/** The Ecosystem view over the sample backend, with the plan-limit settings of the test. */
async function show(settings: Partial<ReturnType<typeof samplePrefs>> = {}, switches: Parameters<typeof freshBackend>[0] = {}) {
  prefs.value = samplePrefs({ motion: 'off', ...settings });
  const backend = freshBackend(switches);
  render(EcosystemView);

  const card = await screen.findByRole('region', { name: 'Plan limits' });
  await within(card).findByRole('meter', { name: /^Claude Max 5-hour limit/ });

  return { backend, card };
}

const STYLES: LimitGaugePref[] = ['bars', 'tach', 'segments', 'vu', 'thermo', 'waffle', 'battery'];

describe('Plan limits card, chart styles', () => {
  it('lists the seven styles by name, and the card keeps drawing the same limits in each', async () => {
    const { card } = await show();

    for (const style of STYLES) {
      const picker = within(card).getByRole('button', { name: /^Chart style: / });
      await fireEvent.click(picker);

      if (style === 'bars') {
        expect(screen.getAllByRole('menuitemradio').map((item) => item.textContent?.trim())).toEqual(['Bars', 'Battery', 'Segmented arc', 'Tachometer', 'Thermometer', 'VU meter', 'Waffle']);
      }

      await fireEvent.click(screen.getByRole('menuitemradio', { name: LABELS[style] }));
      await waitFor(() => expect(prefs.value?.limitGauge).toBe(style));

      const meter = within(card).getByRole('meter', { name: /^Claude Max 5-hour limit: 43% used/ });
      expect(within(card).getAllByRole('meter')).toHaveLength(3);
      expect(within(meter).getByText('43%')).toBeInTheDocument();
      // Only the bars are plain boxes: every other style is a drawing.
      expect(meter.querySelector('svg') !== null).toBe(style !== 'bars');
    }
  });

  it('saves the pick through prefs_set', async () => {
    const { backend, card } = await show();

    await fireEvent.click(within(card).getByRole('button', { name: 'Chart style: Bars' }));
    await fireEvent.click(screen.getByRole('menuitemradio', { name: 'Tachometer' }));

    await waitFor(() => expect(backend.calls('prefs_set').at(-1)?.prefs.limitGauge).toBe('tach'));
    expect(within(card).getByRole('button', { name: 'Chart style: Tachometer' })).toBeInTheDocument();
  });

  it('keeps the picker one width: every style is in the button, only the chosen one is read', async () => {
    const { card } = await show({ limitGauge: 'tach' });

    const picker = within(card).getByRole('button', { name: 'Chart style: Tachometer' });

    expect(picker.textContent).toContain('Segmented arc');
    expect(picker.textContent).toContain('Thermometer');
  });

  it('has no picker when there are no limits to draw', async () => {
    prefs.value = samplePrefs({ motion: 'off' });
    freshBackend({ dashboard: 'failed' });
    render(EcosystemView);

    await screen.findByRole('heading', { level: 2, name: 'Tools that make agents measurable' });

    expect(screen.queryByRole('button', { name: /^Chart style: / })).toBeNull();
  });
});

describe('Plan limits card, used or left', () => {
  it('counts what is used by default, with its own note under the title', async () => {
    const { card } = await show();

    expect(within(card).getByText('How much of each limit is used, and whether it lasts until the reset.')).toBeInTheDocument();
    expect(within(card).getByRole('meter', { name: /^Claude Max Weekly limit: 61% used/ })).toHaveAttribute('aria-valuenow', '61');
  });

  it('counts what is left when asked, in every style', async () => {
    const { card } = await show({ limitBars: 'left' });

    expect(within(card).getByText('How much of each limit is left, and whether it lasts until the reset.')).toBeInTheDocument();
    expect(within(card).getByRole('meter', { name: /^Claude Max 5-hour limit: 57% left/ })).toHaveAttribute('aria-valuetext', '57% left');

    await fireEvent.click(within(card).getByRole('button', { name: 'Chart style: Bars' }));
    await fireEvent.click(screen.getByRole('menuitemradio', { name: 'Battery' }));

    expect(await within(card).findByText('57%')).toBeInTheDocument();
    expect(within(card).getAllByText('left').length).toBeGreaterThan(0);
  });

  it('reads a window that arrives as the share left the same, whatever this app counts', async () => {
    const used = await show({}, { limits: 'left' });

    expect(within(used.card).getByRole('meter', { name: /^Claude Max 5-hour limit: 43% used/ })).toHaveAttribute('aria-valuenow', '43');
    expect(within(used.card).getByRole('meter', { name: /^Claude Max Weekly limit: 61% used/ })).toBeInTheDocument();
  });

  it('reads it the same when this app counts down', async () => {
    const { card } = await show({ limitBars: 'left' }, { limits: 'left' });

    expect(within(card).getByRole('meter', { name: /^Claude Max 5-hour limit: 57% left/ })).toHaveAttribute('aria-valuenow', '57');
  });
});

describe('Plan limits card, the forecast', () => {
  it('explains the pale stretch in a key beside the plan, and says where a limit ends', async () => {
    const { card } = await show();

    // Every plan's page is in the card, the others behind their tab.
    expect(within(card).getAllByText('Used now').length).toBeGreaterThan(0);
    expect(within(card).getAllByText('Used by the reset').length).toBeGreaterThan(0);
    expect(within(card).getAllByText('On track').length).toBeGreaterThan(0);
  });

  it('draws no pale stretch and no key with the forecast off, and keeps the chips', async () => {
    const { card } = await show({ limitForecast: false });

    expect(within(card).queryAllByText('Used now')).toHaveLength(0);
    expect(within(card).queryAllByText('Used by the reset')).toHaveLength(0);
    expect(within(card).getAllByText('On track').length).toBeGreaterThan(0);
  });

  it('names the battery key for what is left at the reset, whatever is counted', async () => {
    const { card } = await show({ limitGauge: 'battery' });

    expect(within(card).getAllByText('Left at reset').length).toBeGreaterThan(0);
    expect(within(card).queryAllByText('Used now')).toHaveLength(0);
  });
});

describe('Plan limits card, many windows', () => {
  it('draws five windows with the short names, a chip where there is a pace and none where the window has no length', async () => {
    const { card } = await show({}, { limits: 'dense' });

    expect(within(card).getAllByRole('meter')).toHaveLength(5);
    expect(within(card).getByRole('meter', { name: /^Claude Max Daily limit: 92% used, Ends \d{2}:\d{2}, resets in/ })).toBeInTheDocument();
    // A provider's own window name has no length: nothing to measure a pace against, so no chip.
    expect(within(card).getByRole('meter', { name: /^Claude Max monthly: 33% used, resets in/ })).toBeInTheDocument();
    expect(within(card).getByText('Fable')).toBeInTheDocument();
  });

  it('draws the dials of five windows too, in the short names', async () => {
    const { card } = await show({ limitGauge: 'segments' }, { limits: 'dense' });

    expect(within(card).getAllByRole('meter')).toHaveLength(5);
    expect(within(card).getByText('Week')).toBeInTheDocument();
    expect(within(card).getByText('Fable')).toBeInTheDocument();
  });

  it('draws a lone window beside its details', async () => {
    const { card } = await show({ limitGauge: 'tach' });

    await fireEvent.click(within(card).getByRole('button', { name: 'Provider: Claude Max' }));
    await fireEvent.click(screen.getByRole('menuitemradio', { name: 'Codex Pro' }));

    // Codex has two windows, one of them reset since it was read.
    expect(within(card).getByRole('meter', { name: 'Codex Pro 5-hour limit: 0% used, Not used yet, has reset since last seen' })).toBeInTheDocument();
    expect(within(card).getByRole('meter', { name: /^Codex Pro Weekly limit: 18% used/ })).toBeInTheDocument();
  });
});

const LABELS: Record<LimitGaugePref, string> = {
  bars: 'Bars',
  tach: 'Tachometer',
  segments: 'Segmented arc',
  vu: 'VU meter',
  thermo: 'Thermometer',
  waffle: 'Waffle',
  battery: 'Battery',
};
