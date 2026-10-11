import { render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';

import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import Logo from './Logo.svelte';

const setMotion = (motion: 'full' | 'reduced' | 'off') => (prefs.value = samplePrefs({ motion }));
const cursorOf = (root: HTMLElement) => root.querySelector('[data-part="cursor"]') as SVGElement;

beforeEach(() => setMotion('full'));

describe('Logo', () => {
  it('paints the whole mark with the logo gradient defined inside its own SVG', () => {
    render(Logo, { props: { size: 32 } });

    const svg = screen.getByRole('img', { name: 'Harness Desktop' }).querySelector('svg') as SVGElement;
    const gradient = svg.querySelector('linearGradient') as SVGElement;

    expect(svg.querySelectorAll('path')).toHaveLength(2);
    expect(svg.querySelector('g')).toHaveAttribute('fill', `url(#${gradient.id})`);
    expect([...gradient.querySelectorAll('stop')].map((stop) => stop.getAttribute('stop-color'))).toEqual(['#38BDF8', '#818CF8']);
  });

  it("wears another family product's gradient when asked", () => {
    render(Logo, { props: { product: 'dashboard' } });

    const stops = [...document.querySelectorAll('linearGradient stop')].map((stop) => stop.getAttribute('stop-color'));
    expect(stops).toEqual(['#F59E0B', '#F472B6']);
  });

  it('gives every instance its own gradient so theme variables resolve in each', () => {
    render(Logo);
    render(Logo);

    const ids = [...document.querySelectorAll('linearGradient')].map((gradient) => gradient.id);

    expect(new Set(ids).size).toBe(ids.length);
  });

  it('shows the name only as a lockup', () => {
    const { container, rerender } = render(Logo, { props: { wordmark: true } });
    expect(container).toHaveTextContent('harness desktop');

    rerender({ wordmark: false });
    expect(container).not.toHaveTextContent('harness');
  });

  it('stays completely still while idle', () => {
    const { container } = render(Logo, { props: { state: 'idle' } });

    expect(cursorOf(container)).not.toHaveClass('blink');
  });

  it('blinks the cursor while an agent is working', () => {
    const { container } = render(Logo, { props: { state: 'working' } });

    expect(screen.getByRole('img')).toHaveAttribute('data-state', 'working');
    expect(cursorOf(container)).toHaveClass('blink');
  });

  it.each(['reduced', 'off'] as const)('does not blink under %s motion', (motion) => {
    setMotion(motion);
    const { container } = render(Logo, { props: { state: 'working' } });

    expect(cursorOf(container)).not.toHaveClass('blink');
  });
});
