import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import Logo from './Logo.svelte';

const setMotion = (motion: 'full' | 'reduced' | 'off') => (prefs.value = samplePrefs({ motion }));
const cursorOf = (root: HTMLElement) => root.querySelector('[data-part="cursor"]') as SVGElement;

beforeEach(() => setMotion('full'));

describe('Logo', () => {
  it('paints the whole mark with a gradient defined inside its own SVG', () => {
    render(Logo, { props: { size: 32 } });

    const svg = screen.getByRole('img', { name: 'Harness Desktop' }).querySelector('svg') as SVGElement;
    const gradient = svg.querySelector('linearGradient') as SVGElement;
    const fills = [...svg.querySelectorAll('path, rect')].map((shape) => shape.getAttribute('fill'));

    expect(gradient).not.toBeNull();
    expect(fills).toHaveLength(2);
    expect(new Set(fills)).toEqual(new Set([`url(#${gradient.id})`]));
    expect([...gradient.querySelectorAll('stop')].map((stop) => (stop as SVGElement).style.stopColor)).toEqual([
      'var(--accent-1)',
      'var(--accent-2)',
      'var(--accent-3)',
    ]);
  });

  it('gives every instance its own gradient so theme variables resolve in each', () => {
    render(Logo);
    render(Logo);

    const ids = [...document.querySelectorAll('linearGradient')].map((gradient) => gradient.id);

    expect(new Set(ids).size).toBe(ids.length);
  });

  it('shows the name only as a lockup', () => {
    const { container, rerender } = render(Logo, { props: { wordmark: true } });
    expect(container).toHaveTextContent('harnessdesktop');

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

  it('plays the launch and calls back once the cursor has finished blinking', async () => {
    const onfinish = vi.fn();
    const { container } = render(Logo, { props: { animate: 'launch', onfinish } });
    const logo = screen.getByRole('img');

    expect(logo).toHaveAttribute('data-launching');
    expect(container.querySelector('.crack')).not.toBeNull();

    // Animations of the cursor's children bubble up; only the cursor's own end finishes the launch.
    await fireEvent(container.querySelector('.leg') as Element, new Event('animationend', { bubbles: true }));
    expect(onfinish).not.toHaveBeenCalled();
    expect(logo).toHaveAttribute('data-launching');

    await fireEvent(cursorOf(container), new Event('animationend', { bubbles: true }));
    expect(onfinish).toHaveBeenCalledTimes(1);
    expect(logo).not.toHaveAttribute('data-launching');
    expect(container.querySelector('.crack')).toBeNull();
  });

  it.each(['reduced', 'off'] as const)('shows the finished mark without any launch motion under %s motion', (motion) => {
    setMotion(motion);
    const onfinish = vi.fn();
    const { container } = render(Logo, { props: { animate: 'launch', wordmark: true, onfinish } });

    expect(screen.getByRole('img')).not.toHaveAttribute('data-launching');
    expect(container.querySelector('.crack, .leg, .puff')).toBeNull();
    expect(container).toHaveTextContent('harnessdesktop');
    expect(onfinish).toHaveBeenCalledTimes(1);
  });
});
