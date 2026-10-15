/** The ids a user has ticked in a list. A change replaces the set, so everything that reads it updates. */
export class IdSelection {
  private chosen = $state.raw(new Set<string>());

  has(id: string): boolean {
    return this.chosen.has(id);
  }

  /** A copy to change and hand back to `replace`. */
  copy(): Set<string> {
    return new Set(this.chosen);
  }

  replace(ids: Iterable<string>): void {
    this.chosen = new Set(ids);
  }

  /** The ticked ids among `items`, in the order the list shows them. */
  inOrder(items: readonly { id: string }[]): string[] {
    return items.filter((item) => this.chosen.has(item.id)).map((item) => item.id);
  }
}
