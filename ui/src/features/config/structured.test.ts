import { describe, expect, it } from 'vitest';

import type { ConfigField } from '$lib/ipc/bindings/ConfigField';
import type { ConfigValue } from '$lib/ipc/bindings/ConfigValue';

import { buildGroups, groupOf, meterBars, parseModelRef, parseRoles, roleMatches, roleSegments } from './structured';

const text = (value: string): ConfigValue => ({ kind: 'text', value });
const map = (value: ConfigField[]): ConfigValue => ({ kind: 'map', value });
const field = (key: string, value: ConfigValue): ConfigField => ({ key, value });

describe('parseModelRef', () => {
  it('splits provider, model and thinking level', () => {
    expect(parseModelRef('openai-codex/gpt-6.1-sol:high')).toEqual({ provider: 'openai-codex', model: 'gpt-6.1-sol', level: 'high' });
  });

  it('has no level when none is written', () => {
    expect(parseModelRef('web/kimi')).toEqual({ provider: 'web', model: 'kimi', level: null });
  });

  it('keeps a colon that is not a thinking level in the model name', () => {
    expect(parseModelRef('ollama/llama3:latest')).toEqual({ provider: 'ollama', model: 'llama3:latest', level: null });
  });

  it('keeps slashes in the model name after the first one', () => {
    expect(parseModelRef('openrouter/anthropic/claude:xhigh')).toEqual({ provider: 'openrouter', model: 'anthropic/claude', level: 'xhigh' });
  });

  it('reads a Claude Code selector as a model and an effort without a provider', () => {
    expect(parseModelRef('claude-opus-5-5:high')).toEqual({ provider: null, model: 'claude-opus-5-5', level: 'high' });
    expect(parseModelRef('opus')).toEqual({ provider: null, model: 'opus', level: null });
    expect(parseModelRef('inherit')).toEqual({ provider: null, model: 'inherit', level: null });
    expect(parseModelRef('sonnet:max')).toEqual({ provider: null, model: 'sonnet', level: 'max' });
    expect(parseModelRef('anthropic/claude-sonnet-5-5:medium')).toEqual({ provider: 'anthropic', model: 'claude-sonnet-5-5', level: 'medium' });
  });

  it('reads a Codex selector as a model and a reasoning effort without a provider', () => {
    expect(parseModelRef('gpt-5.5:high')).toEqual({ provider: null, model: 'gpt-5.5', level: 'high' });
    expect(parseModelRef('gpt-5.5')).toEqual({ provider: null, model: 'gpt-5.5', level: null });
    expect(parseModelRef('gpt-5.5:none')).toEqual({ provider: null, model: 'gpt-5.5', level: 'none' });
    expect(parseModelRef('gpt-5.5:minimal')).toEqual({ provider: null, model: 'gpt-5.5', level: 'minimal' });
    expect(parseModelRef('o3:xhigh')).toEqual({ provider: null, model: 'o3', level: 'xhigh' });
  });

  it('keeps a colon that is not a reasoning effort in the Codex model ID', () => {
    expect(parseModelRef('llama3:latest')).toEqual({ provider: null, model: 'llama3:latest', level: null });
    expect(parseModelRef('llama3:latest:high')).toEqual({ provider: null, model: 'llama3:latest', level: 'high' });
  });

  it('reads inherit as the harness default, with an optional effort of either harness', () => {
    expect(parseModelRef('inherit')).toEqual({ provider: null, model: 'inherit', level: null });
    expect(parseModelRef('inherit:high')).toEqual({ provider: null, model: 'inherit', level: 'high' });
    expect(parseModelRef('inherit:none')).toEqual({ provider: null, model: 'inherit', level: 'none' });
    expect(parseModelRef('inherit:minimal')).toEqual({ provider: null, model: 'inherit', level: 'minimal' });
    expect(parseModelRef('inherit:extreme')).toBeNull();
  });

  it('rejects a Claude Code model with an effort Claude Code does not know', () => {
    expect(parseModelRef('opus:extreme')).toBeNull();
    expect(parseModelRef('opus:high:low')).toBeNull();
    expect(parseModelRef('claude-opus-5-5:none')).toBeNull();
  });

  it('rejects text with spaces, an empty part, or no model', () => {
    expect(parseModelRef('not a model')).toBeNull();
    expect(parseModelRef(':high')).toBeNull();
    expect(parseModelRef('')).toBeNull();
    expect(parseModelRef('/model')).toBeNull();
    expect(parseModelRef('a/')).toBeNull();
    expect(parseModelRef('a/b c')).toBeNull();
  });
});

describe('meterBars', () => {
  it('fills one to four bars and none below low', () => {
    expect(['low', 'medium', 'high', 'xhigh', 'off', 'none', 'minimal'].map(meterBars)).toEqual([1, 2, 3, 4, 0, 0, 0]);
  });

  it('fills every bar for Claude Code\'s max effort', () => {
    expect(meterBars('max')).toBe(4);
  });
});

describe('parseRoles', () => {
  it('lists the default role first and keeps the rest in file order', () => {
    const rows = parseRoles(map([field('plan', text('a/b:high')), field('default', text('c/d')), field('web', text('e/f'))]));

    expect(rows?.map((row) => row.role)).toEqual(['default', 'plan', 'web']);
  });

  it('gives up when any value is not a model reference or not text', () => {
    expect(parseRoles(map([field('a', text('x/y')), field('b', text('not a model'))]))).toBeNull();
    expect(parseRoles(map([field('a', text('x/y')), field('b', { kind: 'bool', value: true })]))).toBeNull();
    expect(parseRoles(text('x/y'))).toBeNull();
  });
});

describe('groupOf', () => {
  it('maps known keys and sends the rest to other, even inherited object keys', () => {
    expect(groupOf('theme')).toBe('appearance');
    expect(groupOf('task')).toBe('behavior');
    expect(groupOf('codexResets')).toBe('developer');
    expect(groupOf('somethingNew')).toBe('other');
    expect(groupOf('constructor')).toBe('other');
  });
});

describe('buildGroups', () => {
  const fields: ConfigField[] = [
    field('zeta', text('z')),
    field('theme', map([field('dark', text('titanium'))])),
    field('hideThinkingBlock', { kind: 'bool', value: true }),
    field('modelRoles', map([field('plan', text('a/b:high')), field('default', text('c/d'))])),
    field('symbolPreset', text('nerd')),
  ];

  it('orders cards as listed and leaves out empty ones', () => {
    expect(buildGroups(fields, '').map((group) => group.id)).toEqual(['modelRoles', 'appearance', 'behavior', 'other']);
  });

  it('puts model roles in a table and scalars apart from structure', () => {
    const [roles, appearance] = buildGroups(fields, '');

    expect(roles.body.kind === 'roles' && roles.body.rows.map((row) => row.role)).toEqual(['default', 'plan']);
    expect(appearance.body.kind === 'rows' && appearance.body.scalars.map((entry) => entry.key)).toEqual(['symbolPreset']);
    expect(appearance.body.kind === 'rows' && appearance.body.nested.map((entry) => entry.key)).toEqual(['theme']);
  });

  it('falls back to the plain tree when a role is not a model reference', () => {
    const odd = [field('modelRoles', map([field('plan', text('a/b')), field('weird', text('???'))]))];
    const [group] = buildGroups(odd, '');

    expect(group.body.kind).toBe('rows');
    expect(group.count).toBe(2);
  });

  it('keeps only what matches the query, by key or plain value', () => {
    expect(buildGroups(fields, 'nerd').map((group) => group.id)).toEqual(['appearance']);
    expect(buildGroups(fields, 'TITAN').map((group) => group.id)).toEqual(['appearance']);
    expect(buildGroups(fields, 'plan').flatMap((group) => (group.body.kind === 'roles' ? group.body.rows.map((row) => row.role) : []))).toEqual(['plan']);
    expect(buildGroups(fields, 'nothing here')).toEqual([]);
  });

  it('never matches a secret by its value', () => {
    const secret: ConfigField[] = [field('apiKey', { kind: 'secret', value: { type: 'value' } })];

    expect(buildGroups(secret, 'set')).toEqual([]);
    expect(buildGroups(secret, 'apikey')).toHaveLength(1);
  });
});

describe('role rows', () => {
  const rows = parseRoles(map([field('default', text('openai-codex/gpt-6.1-sol:high')), field('plan', text('web/kimi'))]))!;
  const shown = (query: string) =>
    buildGroups([field('modelRoles', map([field('default', text('openai-codex/gpt-6.1-sol:high')), field('plan', text('web/kimi'))]))], query).flatMap((group) =>
      group.body.kind === 'roles' ? group.body.rows.map((row) => row.role) : [],
    );

  it('splits a row into the pieces it shows', () => {
    expect(roleSegments(rows[0])).toEqual(['default', 'openai-codex/', 'gpt-6.1-sol', 'high']);
    expect(roleSegments(rows[1])).toEqual(['plan', 'web/', 'kimi']);
  });

  it('keeps a row when the query is inside one of its pieces', () => {
    expect(shown('codex')).toEqual(['default']);
    expect(shown('KIMI')).toEqual(['plan']);
    expect(shown('high')).toEqual(['default']);
    expect(shown('plan')).toEqual(['plan']);
    expect(shown('/')).toEqual(['default', 'plan']);
  });

  it('drops a row whose match would span pieces, since nothing could be highlighted', () => {
    expect(shown('model:high')).toEqual([]);
    expect(shown('sol:high')).toEqual([]);
    expect(shown('/gpt')).toEqual([]);
    expect(roleMatches(rows[0], 'sol:high')).toBe(false);
  });

  it('has no provider piece for a Claude Code selector', () => {
    const [claude] = parseRoles(map([field('default', text('claude-opus-5-5:high'))]))!;

    expect(roleSegments(claude)).toEqual(['default', 'claude-opus-5-5', 'high']);
    expect(roleMatches(claude, '/')).toBe(false);
  });

  it('has no level piece for a row without one', () => {
    expect(roleMatches(rows[1], 'high')).toBe(false);
  });
});
