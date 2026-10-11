import { describe, expect, it } from 'vitest';

import { modelMaker } from './maker';
import { LIMIT_COLORS, generalPool, limitFill, limitInk, makerModelHex, modelShade, poolHex, providerHex } from './palette';

describe('dashboard palette', () => {
  it('gives each harness the color the dashboard gives it, per theme', () => {
    expect(providerHex('claude', 'light')).toBe('#de602c');
    expect(providerHex('Claude', 'dark')).toBe('#f77745');
    expect(providerHex('codex', 'dark')).toBe('#5da3ff');
    expect(providerHex('omp', 'light')).toBe('#9e3db0');
  });

  it('draws a provider without a color of its own in the neutral gray', () => {
    expect(providerHex('xai', 'light')).toBe('#b5b3ab');
    expect(providerHex('xai', 'dark')).toBe('#5f5e59');
  });

  it('keeps the provider color while plenty is left, then warns', () => {
    expect(limitFill('ok', 'claude', 'light')).toBe('#de602c');
    expect(limitFill('low', 'claude', 'light')).toBe(LIMIT_COLORS.serious.light);
    expect(limitFill('critical', 'claude', 'dark')).toBe(LIMIT_COLORS.critical.dark);
    expect(limitInk('low', 'dark')).toBe('#e0a23a');
    expect(limitInk('critical', 'light')).toBe('#b42323');
  });
});

// Expected values come from running the dashboard's own web/src/lib/palette.ts and src/core/models.ts.
describe('dashboard model colors', () => {
  it('starts every maker shade sequence at the provider color', () => {
    expect(modelShade('claude', 'dark', 0)).toBe(providerHex('claude', 'dark'));
    expect(modelShade('claude', 'light', 0)).toBe(providerHex('claude', 'light'));
    expect(modelShade('codex', 'dark', 0)).toBe(providerHex('codex', 'dark'));
  });

  it('generates the same shades the dashboard does', () => {
    const shades = (provider: 'claude' | 'codex', mode: 'light' | 'dark') => [0, 1, 2, 3, 4, 5].map((n) => modelShade(provider, mode, n));

    expect(shades('claude', 'light')).toEqual(['#de602c', '#851700', '#f19f8d', '#b13e00', '#f77e32', '#d68675']);
    expect(shades('claude', 'dark')).toEqual(['#f77745', '#a52205', '#ce4e27', '#f5a290', '#c57766', '#de680f']);
    expect(shades('codex', 'light')).toEqual(['#3c8cf0', '#24467d', '#7cb7ef', '#396ca1', '#1051b4', '#3f6dd6']);
    expect(shades('codex', 'dark')).toEqual(['#5da3ff', '#005ba6', '#4f82b1', '#7fc1ff', '#5985e5', '#386ec9']);
  });

  it('hands the general pool out main colors first, then the variants', () => {
    expect(generalPool('light')).toEqual(['#47a438', '#009cbc', '#8279ee', '#d65895', '#136400', '#005c70', '#4c3bab', '#92125a']);
    expect(generalPool('dark')).toEqual(['#5ebb4f', '#00b4d7', '#9791ff', '#ef6fab', '#85e276', '#51dbff', '#c0c0ff', '#ffaacd']);
  });

  it('folds into the Other gray past the end of the pool', () => {
    expect(poolHex(0, 'light')).toBe('#47a438');
    expect(poolHex(7, 'dark')).toBe('#ffaacd');
    expect(poolHex(8, 'light')).toBe('#b5b3ab');
    expect(poolHex(-1, 'dark')).toBe('#5f5e59');
  });

  it('finds the maker however a harness spells the model', () => {
    expect(modelMaker('claude-opus-4-6[1m]')).toBe('anthropic');
    expect(modelMaker('openrouter/anthropic/claude-opus-5.5')).toBe('anthropic');
    expect(modelMaker('anthropic.claude-3-5-sonnet-v2')).toBe('anthropic');
    expect(modelMaker('claude-haiku-4-5@20251001')).toBe('anthropic');
    expect(modelMaker('gpt-5.1-codex')).toBe('openai');
    expect(modelMaker('o3-mini')).toBe('openai');
    expect(modelMaker('github-copilot/gpt-4.1')).toBe('openai');
    expect(modelMaker('gemini-2.5-pro')).toBeNull();
    expect(modelMaker('deepseek-v3')).toBeNull();
  });

  it('ranks a maker model among its own maker only', () => {
    const peers = ['gpt-5.1-codex', 'claude-opus-4-6', 'deepseek-v3', 'claude-sonnet-4-6', 'o3-mini'];

    expect(makerModelHex('claude-opus-4-6', peers, 'dark')).toBe('#f77745');
    expect(makerModelHex('claude-sonnet-4-6', peers, 'dark')).toBe('#a52205');
    expect(makerModelHex('gpt-5.1-codex', peers, 'light')).toBe('#3c8cf0');
    expect(makerModelHex('o3-mini', peers, 'light')).toBe('#24467d');
    expect(makerModelHex('deepseek-v3', peers, 'light')).toBeNull();
  });
});
