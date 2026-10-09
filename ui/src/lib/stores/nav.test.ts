import { describe, expect, it } from 'vitest';

import { NavStore } from './nav.svelte';

describe('NavStore', () => {
  it('moves back and forward through visited views', () => {
    const nav = new NavStore();

    nav.go('sessions');
    nav.go('config');
    nav.back();
    expect(nav.current).toBe('sessions');
    expect(nav.canGoForward).toBe(true);

    nav.forward();
    expect(nav.current).toBe('config');
    expect(nav.canGoForward).toBe(false);
  });

  it('drops forward history after navigating somewhere new', () => {
    const nav = new NavStore();
    nav.go('sessions');
    nav.back();

    nav.go('changes');

    expect(nav.canGoForward).toBe(false);
    expect(nav.canGoBack).toBe(true);
  });

  it('ignores navigating to the current view and back/forward at the ends', () => {
    const nav = new NavStore();

    nav.go('chat');
    nav.back();
    nav.forward();

    expect(nav.current).toBe('chat');
    expect(nav.canGoBack).toBe(false);
  });

  it('reports the travel direction along the rail order', () => {
    const nav = new NavStore();

    nav.go('config');
    expect(nav.direction).toBe(1);

    nav.go('sessions');
    expect(nav.direction).toBe(-1);
  });
});
