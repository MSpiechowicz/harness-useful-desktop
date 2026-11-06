import { describe, expect, it } from 'vitest';

import { approvalArgument, humanToolName, shortSkillOf, skillApproval, skillOf } from './tool-meta';
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

  it('shows an approval what the skill is given', () => {
    const skill = call('Skill', { skill: 'deploy', args: '--env prod --force' });

    expect(skillApproval(skill)).toBe('deploy --env prod --force');
    expect(shortSkillOf(skill)).toBe('deploy');
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

describe('approvalArgument', () => {
  const call = (name: string, args: ToolCallState['args']) => ({ name, args }) as ToolCallState;

  it('shows where a command runs, not its noise', () => {
    expect(approvalArgument(call('bash', { command: 'rm -rf build', cwd: '/prod', timeout: 5 }))).toBe('{"cwd":"/prod"}');
    expect(approvalArgument(call('bash', { command: 'ls', description: 'list' }))).toBeNull();
  });

  it('hides no argument of another tool behind its main one', () => {
    expect(approvalArgument(call('mcp__x__post', { url: 'https://a', body: 'b' }))).toBe('{"url":"https://a","body":"b"}');
    expect(approvalArgument(call('mcp__x__get', { url: 'https://a' }))).toBe('https://a');
    expect(approvalArgument(call('github', { op: 'delete' }))).toBe('{"op":"delete"}');
  });

  it('shows what a file change writes and keeps a read to its path', () => {
    expect(approvalArgument(call('Write', { file_path: 'a.ts', content: 'x' }))).toBe('{"file_path":"a.ts","content":"x"}');
    expect(approvalArgument(call('Read', { file_path: 'a.ts', limit: 5 }))).toBe('a.ts');
  });

  it('never rewrites the whitespace of an argument', () => {
    expect(approvalArgument(call('mcp__x__run', { command_text: 'x', path: 'a\nb' }))).toContain('a\\nb');
    expect(approvalArgument(call('mcp__x__get', { url: 'a\u2028b' }))).toBe('a\u2028b');
  });
});
