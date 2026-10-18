import { flushSync } from 'svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { DETAILS_INLINE_FROM_PX, DRAWER_BELOW_PX } from '$lib/design/layout';

import { WorkspaceState, type WorkspaceId } from './workspace.svelte';

const NARROW = DRAWER_BELOW_PX - 100;
const MIDDLE = DETAILS_INLINE_FROM_PX - 160;
const WIDE = DETAILS_INLINE_FROM_PX + 140;

const stops: Array<() => void> = [];

/** A workspace made inside an effect root, as a view creates it while it initialises, and laid out at `width`. */
function mount(width: number, { id = 'sessions', details = true }: { id?: WorkspaceId; details?: boolean } = {}) {
  let workspace!: WorkspaceState;
  let detach!: () => void;
  const stop = $effect.root(() => {
    workspace = new WorkspaceState(id, { details });
    workspace.width = width;
    detach = workspace.attach();
  });
  flushSync();
  stops.push(() => detach(), stop);

  return { workspace, detach: () => (detach(), flushSync()) };
}

beforeEach(() => window.localStorage.clear());

afterEach(() => {
  stops.splice(0).forEach((stop) => stop());
  flushSync();
  document.body.replaceChildren();
});

describe('a wide view', () => {
  it('shows both columns, and the toggles hide and show them and remember it', () => {
    const { workspace } = mount(WIDE);

    expect(workspace.listVisible).toBe(true);
    expect(workspace.detailsVisible).toBe(true);

    expect(WorkspaceState.toggleCurrentList()).toBe(true);
    expect(WorkspaceState.toggleCurrentDetails()).toBe(true);
    expect(workspace.listVisible).toBe(false);
    expect(workspace.detailsVisible).toBe(false);
    expect(window.localStorage.getItem('harness.workspace.sessions.list')).toBe('false');
    expect(window.localStorage.getItem('harness.workspace.sessions.details')).toBe('false');

    const again = mount(WIDE);
    expect(again.workspace.listVisible).toBe(false);
    expect(again.workspace.detailsVisible).toBe(false);

    again.workspace.toggleList();
    expect(window.localStorage.getItem('harness.workspace.sessions.list')).toBe('true');
  });

  it('keeps a hidden column per view', () => {
    mount(WIDE, { id: 'config' }).workspace.toggleList();

    expect(mount(WIDE, { id: 'extensions' }).workspace.listVisible).toBe(true);
    expect(mount(WIDE, { id: 'config' }).workspace.listVisible).toBe(false);
  });

  it('counts an unmeasured view as wide', () => {
    const { workspace } = mount(0);

    expect(workspace.listInline).toBe(true);
    expect(workspace.detailsInline).toBe(true);
  });

  it('counts a width that is not a number as unmeasured, not as narrow', () => {
    for (const width of [Number.NaN, Number.POSITIVE_INFINITY, undefined as unknown as number]) {
      const { workspace } = mount(width);

      expect(workspace.listInline).toBe(true);
      expect(workspace.detailsInline).toBe(true);
    }
  });
});

describe('a middle-width view', () => {
  it('keeps the list a column and opens the details over the centre, without saving anything', () => {
    const { workspace } = mount(MIDDLE);

    expect(workspace.listVisible).toBe(true);
    expect(workspace.detailsVisible).toBe(false);

    workspace.toggleDetails();
    expect(workspace.detailsVisible).toBe(true);
    expect(workspace.overlayOpen).toBe(true);

    workspace.toggleDetails();
    expect(workspace.overlayOpen).toBe(false);
    expect(window.localStorage.length).toBe(0);
  });

  it('puts the slide-over away when the window grows wide enough for the column', () => {
    const { workspace } = mount(MIDDLE);
    workspace.toggleDetails();

    workspace.width = WIDE;
    flushSync();

    expect(workspace.overlayOpen).toBe(false);
    expect(workspace.detailsVisible).toBe(true);
  });

  it('closes the slide-over from its own button', () => {
    const { workspace } = mount(MIDDLE);
    workspace.toggleDetails();

    workspace.closeDetails();

    expect(workspace.overlayOpen).toBe(false);
  });
});

describe('a narrow view', () => {
  it('opens the list as a drawer and puts it away, without saving anything', () => {
    const { workspace } = mount(NARROW);
    expect(workspace.listVisible).toBe(false);

    expect(WorkspaceState.toggleCurrentList()).toBe(true);
    expect(workspace.drawer.open).toBe(true);
    expect(workspace.listVisible).toBe(true);

    expect(WorkspaceState.toggleCurrentList()).toBe(true);
    expect(workspace.drawer.open).toBe(false);
    expect(window.localStorage.length).toBe(0);
  });

  it('does not touch a list that was hidden on a wide window', () => {
    const { workspace } = mount(WIDE);
    workspace.toggleList();

    workspace.width = NARROW;
    flushSync();
    workspace.toggleList();

    expect(workspace.drawer.open).toBe(true);
    expect(window.localStorage.getItem('harness.workspace.sessions.list')).toBe('false');
  });
});

describe('the shortcuts', () => {
  it('do nothing while no view is laid out, or when it has no details', () => {
    expect(WorkspaceState.toggleCurrentList()).toBe(false);
    expect(WorkspaceState.toggleCurrentDetails()).toBe(false);

    const changes = mount(WIDE, { id: 'changes', details: false });
    expect(WorkspaceState.toggleCurrentList()).toBe(true);
    expect(WorkspaceState.toggleCurrentDetails()).toBe(false);
    expect(changes.workspace.detailsVisible).toBe(false);

    changes.detach();
    expect(WorkspaceState.toggleCurrentList()).toBe(false);
  });

  it('go to the newest view while an older one has not been unmounted yet', () => {
    const older = mount(WIDE, { id: 'sessions' });
    const newer = mount(WIDE, { id: 'config' });

    WorkspaceState.toggleCurrentList();
    expect(newer.workspace.listVisible).toBe(false);
    expect(older.workspace.listVisible).toBe(true);

    newer.detach();
    WorkspaceState.toggleCurrentList();
    expect(older.workspace.listVisible).toBe(false);
  });

  it('tell the toggles whether a view is laid out', () => {
    const { workspace, detach } = mount(WIDE);
    expect(workspace.attached).toBe(true);

    detach();
    expect(workspace.attached).toBe(false);
  });
});

describe('focus', () => {
  /** A list and a details panel as the layout marks them, each with a button, plus the two toggles. */
  function page(id: WorkspaceId) {
    const button = (name: string, attributes: Record<string, string> = {}, parent: HTMLElement = document.body) => {
      const element = document.createElement('button');
      element.id = name;
      Object.entries({ 'data-workspace': id, ...attributes }).forEach(([key, value]) => element.setAttribute(key, value));
      parent.append(element);
    };
    const panel = (kind: 'list' | 'details') => {
      const element = document.createElement('div');
      element.setAttribute('data-workspace', id);
      element.setAttribute('data-workspace-panel', kind);
      document.body.append(element);

      return element;
    };

    button('list-toggle', { 'data-drawer-toggle': '' });
    button('details-toggle', { 'data-details-toggle': '' });
    button('in-list', {}, panel('list'));
    button('in-details', {}, panel('details'));
    button('elsewhere');

    return (name: string) => document.getElementById(name) as HTMLElement;
  }

  it('moves to the toggle when the column that holds it is hidden', () => {
    const el = page('sessions');
    const { workspace } = mount(WIDE);

    el('in-list').focus();
    workspace.toggleList();
    expect(document.activeElement).toBe(el('list-toggle'));

    el('in-details').focus();
    workspace.toggleDetails();
    expect(document.activeElement).toBe(el('details-toggle'));
  });

  it('moves to the toggle when the slide-over holding it is put away', () => {
    const el = page('sessions');
    const { workspace } = mount(MIDDLE);
    workspace.toggleDetails();

    el('in-details').focus();
    workspace.closeOverlay();

    expect(document.activeElement).toBe(el('details-toggle'));
  });

  it('stays where it is when the panel that goes did not hold it', () => {
    const el = page('sessions');
    const { workspace } = mount(WIDE);

    el('elsewhere').focus();
    workspace.toggleList();
    workspace.toggleDetails();

    expect(document.activeElement).toBe(el('elsewhere'));
  });

  it('hands the drawer focus back to its own toggle, not to the first one of another view on screen', () => {
    const el = page('sessions');
    const other = document.createElement('button');
    other.id = 'other-toggle';
    other.setAttribute('data-workspace', 'config');
    other.setAttribute('data-drawer-toggle', '');
    document.body.prepend(other);
    const { workspace } = mount(NARROW);
    workspace.toggleList();

    workspace.drawer.dismiss();

    expect(document.activeElement).toBe(el('list-toggle'));
  });
});
