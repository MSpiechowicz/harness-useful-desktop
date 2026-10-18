import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { createRawSnippet, flushSync } from 'svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { Inbox } from '@lucide/svelte';

import { CENTRE_MIN_PX } from '$lib/design/layout';

import { viewState } from '$lib/stores/view-state.svelte';

import { restoreClientWidth, stubClientWidth } from '../../../tests/helpers';
import { sidePanelPx } from './side-panel';
import WorkspaceLayout from './WorkspaceLayout.svelte';
import WorkspaceToggles from './WorkspaceToggles.svelte';
import { WorkspaceState } from './workspace.svelte';

const WIDE = 1700;
const MIDDLE = 1400;
const NARROW = 1000;

const stops: Array<() => void> = [];

function text(content: string) {
  return createRawSnippet(() => ({ render: () => `<p>${content}</p>` }));
}

/** A workspace with a layout and its toggles on screen, as a view puts them. */
function show(width: number, { empty = false, details = true, scroll = 'page' }: { empty?: boolean; details?: boolean; scroll?: 'page' | 'own' } = {}) {
  stubClientWidth(width);
  let workspace!: WorkspaceState;
  stops.push(
    $effect.root(() => {
      workspace = new WorkspaceState('config', { details });
    }),
  );

  render(WorkspaceToggles, { workspace, listLabel: 'Files' });
  const view = render(WorkspaceLayout, {
    workspace,
    listLabel: 'Files',
    resizeLabel: 'Resize the list',
    scroll,
    scrollKey: 'test:scroll',
    detailsSubtitle: empty ? undefined : 'settings.json',
    detailsEmpty: empty ? { icon: Inbox, message: 'Pick a file' } : undefined,
    list: text('the list'),
    main: text('the centre'),
    details: text('the details'),
  });
  flushSync();

  return { workspace, view };
}

beforeEach(() => window.localStorage.clear());

afterEach(() => {
  restoreClientWidth();
  stops.splice(0).forEach((stop) => stop());
});

describe('a wide workspace', () => {
  it('lays out the list, the centre and the details, both toggles pressed', () => {
    show(WIDE);

    expect(screen.getByText('the list')).toBeInTheDocument();
    expect(screen.getByText('the centre')).toBeInTheDocument();
    const details = screen.getByRole('region', { name: 'Details' });
    expect(within(details).getByText('the details')).toBeInTheDocument();
    expect(within(details).getByText('settings.json')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Files' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('button', { name: 'Details' })).toHaveAttribute('aria-pressed', 'true');
  });

  it('hides a column from its toggle, remembers it, and keeps the centre in place', async () => {
    show(WIDE);
    const centre = screen.getByText('the centre');

    await fireEvent.click(screen.getByRole('button', { name: 'Details' }));

    expect(screen.getByRole('button', { name: 'Details' })).toHaveAttribute('aria-pressed', 'false');
    expect(screen.queryByRole('region', { name: 'Details' })).toBeNull();
    expect(window.localStorage.getItem('harness.workspace.config.details')).toBe('false');
    expect(screen.getByText('the centre')).toBe(centre);

    await fireEvent.click(screen.getByRole('button', { name: 'Files' }));

    expect(screen.getByText('the list').closest('[inert]')).not.toBeNull();
    expect(screen.getByRole('button', { name: 'Files' })).toHaveAttribute('aria-pressed', 'false');
  });

  it('opens hidden again when the view is shown a second time', () => {
    window.localStorage.setItem('harness.workspace.config.details', 'false');

    show(WIDE);

    expect(screen.getByRole('button', { name: 'Details' })).toHaveAttribute('aria-pressed', 'false');
    expect(screen.queryByRole('region', { name: 'Details' })).toBeNull();
  });

  it('asks for a choice in the details until there is one', () => {
    show(WIDE, { empty: true });

    const details = screen.getByRole('region', { name: 'Details' });
    expect(within(details).getByText('Pick a file')).toBeInTheDocument();
    expect(within(details).queryByText('the details')).toBeNull();
  });

  it('groups the toggles as one layout switch, both of them pressable at once', () => {
    show(WIDE);

    const group = screen.getByRole('group', { name: 'Layout' });

    expect(within(group).getAllByRole('button').map((button) => button.getAttribute('aria-label'))).toEqual(['Files', 'Details']);
    expect(within(group).getAllByRole('button').every((button) => button.getAttribute('aria-pressed') === 'true')).toBe(true);
  });

  it('has no details toggle for a view without details', () => {
    show(WIDE, { details: false });

    expect(screen.getByRole('button', { name: 'Files' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Details' })).toBeNull();
    expect(within(screen.getByRole('group', { name: 'Layout' })).getAllByRole('button')).toHaveLength(1);
    expect(screen.queryByRole('region')).toBeNull();
  });
});

describe('the centre of a workspace', () => {
  it('stays the same element, with what the view put in it, when the view changes who scrolls', async () => {
    const { view } = show(WIDE, { scroll: 'page' });
    const centre = screen.getByText('the centre');

    await view.rerender({ scroll: 'own' });
    expect(screen.getByText('the centre')).toBe(centre);

    await view.rerender({ scroll: 'page' });
    expect(screen.getByText('the centre')).toBe(centre);
  });

  it('gives a page its gutter and rhythm and leaves a child that scrolls itself the whole centre', async () => {
    const { view } = show(WIDE, { scroll: 'page' });
    expect(screen.getByText('the centre').closest('.page-gutter')).not.toBeNull();

    await view.rerender({ scroll: 'own' });

    expect(screen.getByText('the centre').closest('.page-gutter')).toBeNull();
  });
});

describe('the scroll position of the centre', () => {
  /** The user scrolls the centre: its page scroller is the element holding the wrapper the content sits in. */
  async function scrollCentreTo(top: number): Promise<void> {
    const scroller = screen.getByText('the centre').parentElement!.parentElement!;
    scroller.scrollTop = top;
    await fireEvent.scroll(scroller);
  }

  it('is remembered while the centre is the page and not while the view scrolls itself', async () => {
    const { view } = show(WIDE, { scroll: 'own' });

    await scrollCentreTo(120);
    expect(viewState.scrollOf('test:scroll')).toBe(0);

    await view.rerender({ scroll: 'page' });
    await scrollCentreTo(240);

    expect(viewState.scrollOf('test:scroll')).toBe(240);
  });
});

describe('the list column', () => {
  const shownWidth = () => Number(screen.getByRole('separator', { name: 'Resize the list' }).getAttribute('aria-valuenow'));

  it('is shown smaller while the details column would leave the centre too little room, and the chosen width returns with the room', async () => {
    window.localStorage.setItem('harness.workspace.config.details', 'false');
    const { workspace } = show(WIDE);
    workspace.listSize = WIDE - CENTRE_MIN_PX;
    flushSync();
    expect(shownWidth()).toBe(WIDE - CENTRE_MIN_PX);

    await fireEvent.click(screen.getByRole('button', { name: 'Details' }));

    expect(shownWidth()).toBe(WIDE - CENTRE_MIN_PX - sidePanelPx());
    expect(workspace.listSize).toBe(WIDE - CENTRE_MIN_PX);

    await fireEvent.click(screen.getByRole('button', { name: 'Details' }));

    expect(shownWidth()).toBe(WIDE - CENTRE_MIN_PX);
  });
});

describe('a middle-width workspace', () => {
  it('slides the details over the centre from the toggle, and Escape puts them away with focus on the toggle', async () => {
    show(MIDDLE);
    const toggle = screen.getByRole('button', { name: 'Details' });
    expect(screen.queryByRole('region', { name: 'Details' })).toBeNull();
    expect(toggle).toHaveAttribute('aria-pressed', 'false');

    toggle.focus();
    await fireEvent.click(toggle);

    const details = await screen.findByRole('region', { name: 'Details' });
    expect(toggle).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByText('the list')).toBeInTheDocument();

    within(details).getByRole('button', { name: 'Close' }).focus();
    await fireEvent.keyDown(details, { key: 'Escape' });

    await waitFor(() => expect(screen.queryByRole('region', { name: 'Details' })).toBeNull());
    expect(document.activeElement).toBe(toggle);
    expect(window.localStorage.length).toBe(0);
  });
});

describe('a narrow workspace', () => {
  it('keeps the list in a drawer until its toggle opens it, and Escape puts it away with focus on the toggle', async () => {
    show(NARROW);
    const toggle = screen.getByRole('button', { name: 'Files' });
    expect(screen.queryByText('the list')).toBeNull();
    expect(toggle).toHaveAttribute('aria-pressed', 'false');

    toggle.focus();
    await fireEvent.click(toggle);

    const drawer = await screen.findByRole('dialog', { name: 'Files' });
    expect(within(drawer).getByText('the list')).toBeInTheDocument();
    expect(toggle).toHaveAttribute('aria-pressed', 'true');

    await fireEvent.keyDown(drawer, { key: 'Escape' });

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Files' })).toBeNull());
    expect(document.activeElement).toBe(toggle);
    expect(window.localStorage.length).toBe(0);
  });
});

describe('a workspace that is not laid out yet', () => {
  it('shows no toggles', () => {
    let workspace!: WorkspaceState;
    stops.push(
      $effect.root(() => {
        workspace = new WorkspaceState('config', { details: true });
      }),
    );

    render(WorkspaceToggles, { workspace, listLabel: 'Files' });

    expect(screen.queryByRole('button')).toBeNull();
  });
});
