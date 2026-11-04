/** The top of a titled section of a detail pane (`SectionCard`, `FactsCard`). */
export interface SectionHead {
  heading: string;
  /** One line under the heading that says what the section is. */
  hint: string;
  /** No card: the section is told apart by a hairline above it and its small caps heading, so a pane of sections is one flat page. */
  flat?: boolean;
}
