/** Which headings of a list the user folded away. Lives outside the view so the choice survives a visit to another view. */
export class Folded<Id extends string> {
  private folded = $state.raw<ReadonlySet<Id>>(new Set());

  has(id: Id): boolean {
    return this.folded.has(id);
  }

  toggle(id: Id): void {
    const next = new Set(this.folded);

    if (!next.delete(id)) {
      next.add(id);
    }

    this.folded = next;
  }
}
