import { describe, expect, it } from 'vitest';

import { humanToolName, shortSkillOf, skillOf } from './tool-meta';
import type { ToolCallState } from './types';

describe('humanToolName', () => {
  it.each([
    ['mcp__harness-code__code_map', 'Harness Code · Code map'],
    ['harness-code_code_map', 'Harness Code · Code map'],
    ['mcp__harness_code_code_map', 'Harness Code · Code map'],
    ['mcp__harness_memory_note_save', 'Harness Memory · Note save'],
    ['mcp__plugin_some-plugin_harness-memory__note_save', 'Harness Memory · Note save'],
    ['harness-knowledge_knowledge_get', 'Harness Knowledge · Knowledge get'],
    ['mcp__harness-dashboard__usage_summary', 'Harness Dashboard · Usage summary'],
    ['mcp__harness-desktop__hook_add', 'Harness Desktop · Hook add'],
    ['mcp__evil_harness_memory__note_save', 'Evil harness memory · Note save'],
    ['mcp__harness-other__do_thing', 'Harness other · Do thing'],
    ['web_search', 'Web search'],
    ['bash', 'Bash'],
    ['mcp__', 'mcp__'],
  ])('%s → %s', (name, expected) => {
    expect(humanToolName(name)).toBe(expected);
  });
});

describe('skillOf', () => {
  const call = (name: string, args: ToolCallState['args']) => ({ name, args }) as ToolCallState;

  it('keeps the plugin for an approval and drops it on a card line', () => {
    const skill = call('Skill', { skill: 'some-plugin:us-grill-me' });

    expect(skillOf(skill)).toBe('some-plugin:us-grill-me');
    expect(shortSkillOf(skill)).toBe('us-grill-me');
  });

  it('reads the argument that harness runs', () => {
    expect(skillOf(call('skill', { skill: 'shown', name: 'runs' }))).toBe('runs');
    expect(skillOf(call('Skill', { skill: 'runs', name: 'shown' }))).toBe('runs');
  });

  it('never gives an empty name', () => {
    expect(shortSkillOf(call('Skill', { skill: 'some-plugin:' }))).toBe('some-plugin:');
  });

  it('leaves other tools and a call that names nothing alone', () => {
    expect(skillOf(call('Read', { name: 'a.ts' }))).toBeNull();
    expect(skillOf(call('Skill', {}))).toBeNull();
    expect(shortSkillOf(call('Skill', null))).toBeNull();
  });
});
