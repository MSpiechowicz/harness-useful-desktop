/** What a team preview looks like: whether team documents run without approval, whether the repository can be shared, and the latest refresh's problem. */
export interface TeamMode {
  trust: boolean;
  shareable: boolean;
  problem: string | null;
}

/** The team of a `?hooks=` or `?routines=` preview by its name: documents are shared in these four, and not in any other. */
export function teamModeOf(scenario: string): TeamMode | null {
  switch (scenario) {
    case 'team':
      return { trust: false, shareable: true, problem: null };
    case 'teamTrusted':
      return { trust: true, shareable: true, problem: null };
    case 'teamNoOrigin':
      return { trust: false, shareable: false, problem: null };
    case 'teamProblem':
      return { trust: false, shareable: true, problem: 'The remote database did not answer in time.' };
    default:
      return null;
  }
}
