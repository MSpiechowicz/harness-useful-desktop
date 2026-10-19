export const VIEW_IDS = ['chat', 'sessions', 'changes', 'config', 'extensions', 'ecosystem', 'notes', 'scripts', 'hooks', 'routines', 'settings'] as const;

export type ViewId = (typeof VIEW_IDS)[number];

/** Current view plus browser-style back/forward history. */
export class NavStore {
  current = $state<ViewId>('chat');
  /** +1 when the last move went deeper in the rail order, -1 when it went up; drives transition direction. */
  direction = $state<1 | -1>(1);

  private past = $state<ViewId[]>([]);
  private future = $state<ViewId[]>([]);

  canGoBack: boolean = $derived(this.past.length > 0);
  canGoForward: boolean = $derived(this.future.length > 0);

  go(view: ViewId): void {
    if (view === this.current) {
      return;
    }

    this.past = [...this.past, this.current];
    this.future = [];
    this.moveTo(view);
  }

  back(): void {
    const previous = this.past[this.past.length - 1];
    if (!previous) {
      return;
    }

    this.future = [this.current, ...this.future];
    this.past = this.past.slice(0, -1);
    this.moveTo(previous);
  }

  forward(): void {
    const next = this.future[0];
    if (!next) {
      return;
    }

    this.past = [...this.past, this.current];
    this.future = this.future.slice(1);
    this.moveTo(next);
  }

  private moveTo(view: ViewId): void {
    this.direction = VIEW_IDS.indexOf(view) >= VIEW_IDS.indexOf(this.current) ? 1 : -1;
    this.current = view;
  }
}

export const nav = new NavStore();
