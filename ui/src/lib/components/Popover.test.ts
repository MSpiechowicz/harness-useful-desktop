import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import DocSelect from './docs/DocSelect.svelte';

const WINDOW = { width: 1024, height: 800 };
const BORDER = 1;

/** The trigger's box in the window. */
let anchor = { left: 100, top: 600, width: 400, height: 36 };

/** The menu wraps its lines at a narrow width: wide it is 100 px tall, at the trigger's width 300 px. */
function contentHeight(width: number): number {
  return width >= 600 ? 100 : 300;
}

function isPanel(element: Element): element is HTMLElement {
  return element instanceof HTMLElement && element.getAttribute('role') === 'menu';
}

/** happy-dom does not lay out: the panel is as wide as its inline width, else as wide as the window (`w-full`). */
function panelWidth(element: HTMLElement): number {
  return element.style.width ? parseFloat(element.style.width) : WINDOW.width;
}

beforeEach(() => {
  anchor = { left: 100, top: 600, width: 400, height: 36 };
  vi.spyOn(window, 'innerWidth', 'get').mockReturnValue(WINDOW.width);
  vi.spyOn(window, 'innerHeight', 'get').mockReturnValue(WINDOW.height);
  vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockImplementation(function (this: HTMLElement) {
    return isPanel(this) ? panelWidth(this) : 0;
  });
  vi.spyOn(Element.prototype, 'scrollHeight', 'get').mockImplementation(function (this: Element) {
    return isPanel(this) ? contentHeight(panelWidth(this)) : 0;
  });
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
    const box = this.querySelector(':scope > [data-popover-trigger]') ? anchor : { left: 0, top: 0, width: 0, height: 0 };

    return DOMRect.fromRect({ x: box.left, y: box.top, width: box.width, height: box.height });
  });

  const computed = window.getComputedStyle.bind(window);
  vi.spyOn(window, 'getComputedStyle').mockImplementation((element: Element) => {
    const style = computed(element);
    if (!isPanel(element)) {
      return style;
    }

    return new Proxy(style, {
      get: (target, key) => (key === 'borderTopWidth' || key === 'borderBottomWidth' ? `${BORDER}px` : Reflect.get(target, key)),
    });
  });
});

afterEach(() => vi.restoreAllMocks());

/** Every inline style the menu panel has had, oldest first, so a placement shown for a moment is checked too. */
function watchPanelStyles(): () => CSSStyleDeclaration[] {
  const seen: string[] = [];
  const observer = new MutationObserver((records) => {
    for (const record of records) {
      if (record.type === 'attributes' && isPanel(record.target as Element) && record.oldValue !== null) {
        seen.push(record.oldValue);
      }
    }
  });
  observer.observe(document.body, { subtree: true, attributes: true, attributeFilter: ['style'], attributeOldValue: true });

  return () => {
    const panel = screen.getByRole('menu');
    observer.disconnect();

    return [...seen, panel.getAttribute('style') ?? ''].map((text) => {
      const style = document.createElement('div').style;
      style.cssText = text;

      return style;
    });
  };
}

async function openMenu(): Promise<HTMLElement> {
  render(DocSelect, {
    props: {
      label: 'When',
      options: [
        { value: 'daily', label: 'Daily', hint: 'Runs once a day at the time you pick' },
        { value: 'hourly', label: 'Hourly', hint: 'Runs at the start of every hour' },
      ],
      value: 'daily',
      labelledby: 'when-label',
      onchange: () => {},
    },
  });

  await fireEvent.click(screen.getByRole('button', { expanded: false }));
  await tick();
  await tick();

  return screen.getByRole('menu', { name: 'When' });
}

describe('Popover side="auto"', () => {
  it('measures the panel at the trigger width, so a menu opening upward ends above its trigger', async () => {
    const styles = watchPanelStyles();
    const panel = await openMenu();

    for (const style of styles().filter((each) => each.top !== '')) {
      expect(style.width).toBe(`${anchor.width}px`);
    }

    const top = parseFloat(panel.style.top);
    const shown = Math.min(contentHeight(anchor.width) + BORDER * 2, parseFloat(panel.style.maxHeight));

    expect(panel.style.width).toBe(`${anchor.width}px`);
    expect(top + shown).toBeLessThanOrEqual(anchor.top);
    expect(top + shown).toBe(anchor.top - 8);
  });

  it('keeps a panel as wide as its trigger inside the window when the trigger sits at the right edge', async () => {
    anchor = { left: 900, top: 100, width: 200, height: 36 };

    const panel = await openMenu();

    expect(parseFloat(panel.style.left)).toBe(WINDOW.width - 8 - anchor.width);
    expect(parseFloat(panel.style.top)).toBe(anchor.top + anchor.height + 8);
  });
});
