import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { i18n } from '$lib/i18n/i18n.svelte';
import type { ConfigField } from '$lib/ipc/bindings/ConfigField';
import type { ConfigValue } from '$lib/ipc/bindings/ConfigValue';

import { countMatches, flattenTree, toggleableIds } from './config-tree';
import ConfigTree from './ConfigTree.svelte';

const text = (value: string): ConfigValue => ({ kind: 'text', value });
const map = (value: ConfigField[]): ConfigValue => ({ kind: 'map', value });

/** A secret as the backend sends it, plus a poisoned copy carrying a value it must never have. */
const LEAK = 'sk-live-do-not-show';
const poisoned = { kind: 'secret', value: { type: 'value', leaked: LEAK } } as unknown as ConfigValue;

const FIELDS: ConfigField[] = [
  { key: 'model', value: text('claude-sonnet') },
  { key: 'retries', value: { kind: 'number', value: '3' } },
  { key: 'streaming', value: { kind: 'bool', value: true } },
  { key: 'fallback', value: { kind: 'null' } },
  {
    key: 'providers',
    value: map([
      {
        key: 'openai',
        value: map([
          { key: 'apiKey', value: { kind: 'secret', value: { type: 'envRef', name: 'OPENAI_API_KEY' } } },
          { key: 'token', value: poisoned },
          { key: 'helper', value: { kind: 'secret', value: { type: 'command' } } },
        ]),
      },
    ]),
  },
  { key: 'systemPrompt', value: text('First line of a long prompt.\nSecond line.') },
];

describe('ConfigTree', () => {
  it('speaks Polish: labels, hints and the plural of the entries a branch holds', async () => {
    await i18n.load('pl');

    const entries = (count: number): ConfigField[] => Array.from({ length: count }, (_, index) => ({ key: `k${index}`, value: text('v') }));
    const fields: ConfigField[] = [
      { key: 'two', value: map(entries(2)) },
      { key: 'five', value: map(entries(5)) },
      { key: 'none', value: map([]) },
      { key: 'helper', value: { kind: 'secret', value: { type: 'command' } } },
    ];
    const { unmount } = render(ConfigTree, { props: { fields, label: 'Ustawienia', defaultDepth: 0 } });

    expect(screen.getByText('2 klucze')).toBeInTheDocument();
    expect(screen.getByText('5 kluczy')).toBeInTheDocument();
    expect(screen.getByText('pusto')).toBeInTheDocument();
    expect(screen.getByText('polecenie (ukryte)')).toBeInTheDocument();

    unmount();
    render(ConfigTree, { props: { fields: [], label: 'Ustawienia' } });

    expect(screen.getByRole('status')).toHaveTextContent('Brak wpisów w tym pliku.');
  });

  it('opens and closes every branch and long text at once', async () => {
    const { component } = render(ConfigTree, { props: { fields: FIELDS, label: 'Settings' } });
    const tree = screen.getByRole('tree');

    component.setAll(true);
    await Promise.resolve();
    expect(within(tree).getByText('apiKey')).toBeInTheDocument();
    expect(tree.querySelectorAll('[aria-expanded="false"]')).toHaveLength(0);

    component.setAll(false);
    await Promise.resolve();
    expect(within(tree).queryByText('openai')).toBeNull();
    expect(tree.querySelectorAll('[aria-expanded="true"]')).toHaveLength(0);
  });

  it('lists every row that can open, at any depth', () => {
    expect(toggleableIds(FIELDS)).toEqual(['providers', 'providers\u001fopenai', 'systemPrompt']);
  });

  it('renders each kind of secret as a lock chip and never a value', async () => {
    const { container } = render(ConfigTree, { props: { fields: FIELDS, label: 'Settings', defaultDepth: 3 } });

    expect(screen.getByText('$OPENAI_API_KEY')).toBeInTheDocument();
    expect(screen.getByText('•••• set')).toBeInTheDocument();
    expect(screen.getByText('command (hidden)')).toBeInTheDocument();
    expect(container.textContent).not.toContain(LEAK);
  });

  it('explains an environment reference in its tooltip without reading it', async () => {
    render(ConfigTree, { props: { fields: FIELDS, label: 'Settings', defaultDepth: 3 } });

    // The tooltip wrapper is the chip's parent; hovering the chip bubbles `pointerenter` semantics to it.
    const chip = screen.getByText('$OPENAI_API_KEY').parentElement!;
    await fireEvent.pointerEnter(chip.parentElement!);

    expect(await screen.findByRole('tooltip', undefined, { timeout: 1500 })).toHaveTextContent(
      'read from environment variable OPENAI_API_KEY — value never read',
    );
  });

  it('colors and prints plain values by type', () => {
    render(ConfigTree, { props: { fields: FIELDS, label: 'Settings' } });

    expect(screen.getByText('claude-sonnet')).toHaveClass('text-success');
    expect(screen.getByText('3')).toHaveClass('text-warning');
    expect(screen.getByText('true')).toHaveClass('text-accent-text');
    expect(screen.getByText('null')).toHaveClass('text-fg-3');
  });

  it('opens top-level maps and keeps nested ones closed until expanded', async () => {
    render(ConfigTree, { props: { fields: FIELDS, label: 'Settings' } });

    expect(screen.getByRole('treeitem', { name: /openai/ })).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByText('apiKey')).toBeNull();

    await fireEvent.click(screen.getByRole('treeitem', { name: /openai/ }));

    expect(screen.getByRole('treeitem', { name: /openai/ })).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByText('apiKey')).toBeInTheDocument();
  });

  it('clips long text until it is expanded', async () => {
    render(ConfigTree, { props: { fields: FIELDS, label: 'Settings' } });
    const row = screen.getByRole('treeitem', { name: /systemPrompt/ });

    expect(within(row).getByText(/First line/)).toHaveClass('truncate');

    await fireEvent.click(row);

    expect(within(row).getByText(/First line/)).toHaveClass('whitespace-pre-wrap');
  });

  it('moves with the arrow keys, expanding and collapsing with right and left', async () => {
    render(ConfigTree, { props: { fields: FIELDS, label: 'Settings' } });
    const tree = screen.getByRole('tree', { name: 'Settings' });
    const rows = () => screen.getAllByRole('treeitem');

    rows()[0].focus();
    expect(rows()[0]).toHaveAttribute('tabindex', '0');

    await fireEvent.keyDown(tree, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(rows()[1]);

    const providers = screen.getByRole('treeitem', { name: /providers/ });
    providers.focus();
    await fireEvent.keyDown(tree, { key: 'ArrowLeft' });
    expect(providers).toHaveAttribute('aria-expanded', 'false');

    await fireEvent.keyDown(tree, { key: 'ArrowRight' });
    expect(providers).toHaveAttribute('aria-expanded', 'true');

    await fireEvent.keyDown(tree, { key: 'ArrowRight' });
    expect(document.activeElement).toBe(screen.getByRole('treeitem', { name: /openai/ }));

    await fireEvent.keyDown(tree, { key: 'ArrowLeft' });
    expect(document.activeElement).toBe(providers);
  });

  it('filters by key or value and opens the path to a match', () => {
    render(ConfigTree, { props: { fields: FIELDS, label: 'Settings', query: 'openai' } });

    expect(screen.getByRole('treeitem', { name: /providers/ })).toBeInTheDocument();
    expect(screen.getByRole('treeitem', { name: /openai/ })).toBeInTheDocument();
    expect(screen.queryByText('claude-sonnet')).toBeNull();
  });

  it('never matches a secret by its value, even a leaked one', () => {
    const { container } = render(ConfigTree, { props: { fields: FIELDS, label: 'Settings', query: LEAK } });

    expect(screen.queryAllByRole('treeitem')).toHaveLength(0);
    expect(screen.getByRole('status')).toHaveTextContent('No entries match your filter.');
    expect(container.textContent).not.toContain(LEAK);
  });

  it('does not match the chip text of a secret', () => {
    expect(countMatches(FIELDS, '•••')).toBe(0);
    expect(countMatches(FIELDS, 'command (hidden)')).toBe(0);
    expect(flattenTree(FIELDS, { query: 'hidden', overrides: new Map(), defaultDepth: 3 })).toEqual([]);
  });
});
