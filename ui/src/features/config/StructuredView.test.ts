import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { ConfigField } from '$lib/ipc/bindings/ConfigField';
import type { ConfigValue } from '$lib/ipc/bindings/ConfigValue';

import StructuredView from './StructuredView.svelte';

const text = (value: string): ConfigValue => ({ kind: 'text', value });
const map = (value: ConfigField[]): ConfigValue => ({ kind: 'map', value });
const field = (key: string, value: ConfigValue): ConfigField => ({ key, value });

const LEAK = 'sk-live-do-not-show';

const FIELDS: ConfigField[] = [
  field('modelRoles', map([field('plan', text('openai-codex/gpt-6-astra:high')), field('default', text('opencode-go/longcat')), field('review', text('openai-codex/gpt-6.1-sol:medium'))])),
  field('hideThinkingBlock', { kind: 'bool', value: true }),
  field('symbolPreset', text('nerd')),
  field('setupVersion', { kind: 'number', value: '17' }),
  field('task', map([field('agentModelOverrides', map([field('explore', text('x/y'))]))])),
  field('apiKey', { kind: 'secret', value: { type: 'value', leaked: LEAK } as never }),
  field('futureThing', text('hello')),
];

function mount(props: { fields?: ConfigField[]; query?: string } = {}) {
  return render(StructuredView, { props: { fields: FIELDS, query: '', label: 'Settings', format: 'yaml', ...props } });
}

describe('StructuredView', () => {
  it('shows model roles as a table with the default role first and a thinking meter', () => {
    mount();
    const table = screen.getByRole('table', { name: 'Model roles' });
    const rows = within(table).getAllByRole('row').slice(1);

    expect(rows.map((row) => within(row).getAllByRole('rowheader')[0].textContent?.trim())).toEqual(['default', 'plan', 'review']);
    expect(within(rows[1]).getByText('gpt-6-astra')).toHaveClass('text-success');
    expect(within(rows[1]).getByText('openai-codex/')).toHaveClass('text-fg-3');
    expect(within(rows[1]).getByText('high')).toBeInTheDocument();
    expect(within(rows[0]).getByText('—')).toBeInTheDocument();
    expect(rows[1].querySelectorAll('i.bg-accent')).toHaveLength(3);
    expect(rows[2].querySelectorAll('i.bg-accent')).toHaveLength(2);
  });

  it('falls back to the tree when a role is not a model reference', () => {
    mount({ fields: [field('modelRoles', map([field('plan', text('openai/gpt:high')), field('weird', text('not a model'))]))] });

    expect(screen.queryByRole('table')).toBeNull();
    expect(screen.getByRole('tree', { name: 'Model roles' })).toBeInTheDocument();
    expect(screen.getByText('not a model')).toBeInTheDocument();
  });

  it('shows a flag as a read-only switch named by its key', async () => {
    mount();
    const flag = screen.getByRole('switch', { name: 'hideThinkingBlock' });

    expect(flag).toBeDisabled();
    expect(flag).toHaveAttribute('aria-checked', 'true');

    await fireEvent.click(flag);
    expect(flag).toHaveAttribute('aria-checked', 'true');
  });

  it('prints text as a chip and numbers by type, and keeps unknown keys in Other', () => {
    mount();

    expect(screen.getByText('nerd')).toHaveClass('text-success');
    expect(screen.getByText('17')).toHaveClass('text-warning');
    expect(within(screen.getByRole('region', { name: 'Other' })).getByText('futureThing')).toBeInTheDocument();
  });

  it('hands nested structure to the tree', () => {
    mount();

    expect(screen.getByRole('tree', { name: 'Behavior' })).toBeInTheDocument();
    expect(screen.getByRole('treeitem', { name: /agentModelOverrides/ })).toBeInTheDocument();
  });

  it('never puts a secret value in the page', () => {
    const { container } = mount();

    expect(screen.getByText('•••• set')).toBeInTheDocument();
    expect(container.textContent).not.toContain(LEAK);

    const search = mount({ query: LEAK });
    expect(search.container.textContent).not.toContain(LEAK);
  });

  it('hides what does not match and keeps cards open while searching', () => {
    mount({ query: 'astra' });

    expect(screen.getByRole('row', { name: /plan.*astra/ })).toBeInTheDocument();
    expect(screen.queryByText('default')).toBeNull();
    expect(screen.queryByText('nerd')).toBeNull();
    expect(screen.queryByRole('switch')).toBeNull();
    expect(screen.getByRole('button', { name: /Model roles/ })).toBeDisabled();
    expect(screen.getAllByRole('button', { expanded: true })).toHaveLength(1);
  });

  it('says so when nothing matches', () => {
    mount({ query: 'zzzz' });

    expect(screen.getByRole('status')).toHaveTextContent('No entries match your filter.');
  });

  it('folds a card from its heading', async () => {
    mount();
    const heading = screen.getByRole('button', { name: /Appearance/ });

    expect(heading).toHaveAttribute('aria-expanded', 'true');

    await fireEvent.click(heading);

    expect(heading).toHaveAttribute('aria-expanded', 'false');
    expect(screen.getByText('nerd')).not.toBeVisible();
  });

  it('opens and closes every card and tree row at once and reports how many are open', async () => {
    const { component } = mount();
    const cards = () => screen.getAllByRole('button', { expanded: true });

    expect(cards()).toHaveLength(4);

    component.setAll(false);
    await Promise.resolve();
    expect(screen.queryAllByRole('button', { expanded: true })).toHaveLength(0);
    expect(screen.getByRole('treeitem', { name: /task/, hidden: true })).toHaveAttribute('aria-expanded', 'false');

    component.setAll(true);
    await Promise.resolve();
    expect(cards()).toHaveLength(4);
    expect(screen.getByRole('treeitem', { name: /agentModelOverrides/ })).toHaveAttribute('aria-expanded', 'true');
  });
});
