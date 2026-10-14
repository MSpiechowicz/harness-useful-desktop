import type { ExtTab } from './extension-items';

/** A group longer than this starts folded, so a hundred themes do not push everything else off the screen. */
const LONG_GROUP = 20;

/** Which headings of the inventory the user opened or folded; outside the view so it survives a visit to another view. */
class GroupFolds {
  private chosen = $state.raw<ReadonlyMap<ExtTab, boolean>>(new Map());

  isOpen(tab: ExtTab, total: number): boolean {
    return this.chosen.get(tab) ?? total <= LONG_GROUP;
  }

  /** Back to the defaults: every group as long as it is. */
  reset(): void {
    this.chosen = new Map();
  }

  toggle(tab: ExtTab, total: number): void {
    const next = new Map(this.chosen);

    next.set(tab, !this.isOpen(tab, total));
    this.chosen = next;
  }
}

export const groupFolds = new GroupFolds();
