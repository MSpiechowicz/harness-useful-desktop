import type { HarnessSummary } from '../bindings/HarnessSummary';

/** The version the backend detects for OpenCode 2. */
export const OPENCODE_2_VERSION = '2.1.2';

/** The harness summary with OpenCode detected as its second major version; any other harness is returned as it is. The recorded fixture stays OpenCode 1. */
export function withOpenCode2(summary: HarnessSummary): HarnessSummary {
  return summary.info.id === 'opencode' ? { ...summary, detection: { ...summary.detection, version: OPENCODE_2_VERSION } } : summary;
}
