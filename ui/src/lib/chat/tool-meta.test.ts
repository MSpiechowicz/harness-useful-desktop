import { describe, expect, it } from 'vitest';

import { humanToolName } from './tool-meta';

describe('humanToolName', () => {
  it.each([
    ['mcp__harness_memory_note_search', 'Harness memory note search'],
    ['mcp__harness-dashboard__usage_summary', 'Harness dashboard · Usage summary'],
    ['web_search', 'Web search'],
    ['bash', 'Bash'],
    ['mcp__', 'mcp__'],
  ])('%s → %s', (name, expected) => {
    expect(humanToolName(name)).toBe(expected);
  });
});
