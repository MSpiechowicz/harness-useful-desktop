import { describe, expect, it } from 'vitest';

import type { ConfigField } from '$lib/ipc/bindings/ConfigField';

import { configStats } from './stats';

const text = (key: string): ConfigField => ({ key, value: { kind: 'text', value: 'x' } });

describe('configStats', () => {
  it('counts nothing for an empty file', () => {
    expect(configStats([])).toEqual({ entries: 0, values: 0, depth: 0, secrets: 0 });
  });

  it('calls a flat file one level deep', () => {
    expect(configStats([text('a'), text('b')])).toEqual({ entries: 2, values: 2, depth: 1, secrets: 0 });
  });

  it('counts leaves and levels through maps and lists', () => {
    const fields: ConfigField[] = [
      text('theme'),
      { key: 'roles', value: { kind: 'map', value: [text('default'), text('plan')] } },
      { key: 'servers', value: { kind: 'list', value: [{ kind: 'map', value: [text('name'), { key: 'env', value: { kind: 'map', value: [text('A')] } }] }] } },
    ];

    expect(configStats(fields)).toEqual({ entries: 3, values: 5, depth: 4, secrets: 0 });
  });

  it('counts an empty container as one value and masked secrets separately', () => {
    const fields: ConfigField[] = [
      { key: 'empty', value: { kind: 'list', value: [] } },
      { key: 'auth', value: { kind: 'map', value: [{ key: 'token', value: { kind: 'secret', value: { type: 'value' } } }, text('user')] } },
    ];

    expect(configStats(fields)).toEqual({ entries: 2, values: 3, depth: 2, secrets: 1 });
  });
});
