import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { ConfigField } from '$lib/ipc/bindings/ConfigField';
import type { ConfigFormat } from '$lib/ipc/bindings/ConfigFormat';
import type { ConfigValue } from '$lib/ipc/bindings/ConfigValue';

import SourceView from './SourceView.svelte';

const text = (value: string): ConfigValue => ({ kind: 'text', value });
const map = (value: ConfigField[]): ConfigValue => ({ kind: 'map', value });
const field = (key: string, value: ConfigValue): ConfigField => ({ key, value });

const LEAK = 'sk-live-do-not-show';

const FIELDS: ConfigField[] = [
  field('name', text('omp')),
  field('theme', map([field('dark', text('titanium')), field('light', text('light'))])),
  field('apiKey', { kind: 'secret', value: { type: 'value', leaked: LEAK } as never }),
  field('roles', map([field('plan', text('openai/gpt:high')), field('review', text('openai/gpt:high'))])),
];

function mount(props: { fields?: ConfigField[]; query?: string; format?: ConfigFormat } = {}) {
  return render(SourceView, { props: { fields: FIELDS, query: '', label: 'Source', format: 'yaml', ...props } });
}

/** A line as it reads on screen: its number, then the pieces of its content (the template's own whitespace does not render). */
function printed(item: HTMLElement): string {
  const line = item.querySelector('[data-index]')!;
  const pieces = Array.from(line.lastElementChild!.children).map((piece) => piece.textContent);

  return `${line.firstElementChild!.textContent} ${pieces.join('')}`;
}

const lineOf = (key: string): HTMLElement => screen.getByText(key).closest('[data-index]') as HTMLElement;

describe('SourceView', () => {
  it('prints the file with a number on every line', () => {
    mount();
    const lines = within(screen.getByRole('list', { name: 'Source' })).getAllByRole('listitem');

    expect(lines).toHaveLength(8);
    expect(printed(lines[0])).toBe('1 name: omp');
    expect(printed(lines[2])).toBe('3 dark: titanium');
    expect(printed(lines[4])).toContain('apiKey: ');
  });

  it('prints JSON with braces, quoted keys and commas', () => {
    mount({ format: 'json' });
    const rows = within(screen.getByRole('list', { name: 'Source' })).getAllByRole('listitem');

    expect(printed(rows[0])).toBe('1 {');
    expect(printed(rows[1])).toBe('2 "name": "omp",');
  });

  it('prints TOML with `key = value` lines and table headers, not YAML', () => {
    mount({
      format: 'toml',
      fields: [field('model', text('gpt-5.6-terra')), field('mcp_servers', map([field('memory', map([field('command', text('harness-memory'))]))]))],
    });
    const rows = within(screen.getByRole('list', { name: 'Source' })).getAllByRole('listitem');

    expect(rows.map(printed)).toEqual(['1 model = "gpt-5.6-terra"', '2 ', '3 [mcp_servers.memory]', '4 command = "harness-memory"']);
  });

  it('folds a TOML table under its header', async () => {
    mount({ format: 'toml', fields: [field('mcp_servers', map([field('memory', map([field('command', text('harness-memory'))]))]))] });

    await fireEvent.click(screen.getByRole('button', { name: 'Fold block at line 1' }));

    expect(screen.queryByText('command')).toBeNull();
    expect(screen.getByRole('button', { name: '1 key' })).toBeInTheDocument();
  });

  it('shows a secret as a lock chip and never puts its value in the page', () => {
    const { container } = mount();

    expect(screen.getByText('•••• set')).toBeInTheDocument();
    expect(container.textContent).not.toContain(LEAK);

    const search = mount({ query: LEAK });
    expect(search.container.textContent).not.toContain(LEAK);
  });

  it('folds a block to its first line and a count, and unfolds it again', async () => {
    mount();

    await fireEvent.click(screen.getByRole('button', { name: 'Fold block at line 2' }));

    expect(screen.queryByText('titanium')).toBeNull();
    expect(screen.getByText('2 keys')).toBeInTheDocument();

    await fireEvent.click(screen.getByText('2 keys'));

    expect(screen.getByText('titanium')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Fold block at line 2' })).toHaveAttribute('aria-expanded', 'true');
  });

  it('opens and closes every block at once and reports how many are open', async () => {
    const { component } = mount();

    component.setAll(false);
    await Promise.resolve();
    expect(screen.queryByText('titanium')).toBeNull();
    expect(screen.getAllByRole('button', { expanded: false })).toHaveLength(2);

    component.setAll(true);
    await Promise.resolve();
    expect(screen.getByText('titanium')).toBeInTheDocument();
    expect(screen.getAllByRole('button', { expanded: true })).toHaveLength(2);
  });

  it('marks matching lines and opens a folded block that holds one', async () => {
    const { component } = mount({ query: 'titan' });
    component.setAll(false);
    await Promise.resolve();

    expect(screen.getByText('titan')).toBeInTheDocument();
    expect(lineOf('dark')).toHaveClass('bg-warning-wash');
    expect(lineOf('name')).not.toHaveClass('bg-warning-wash');
    // Only the block without a match stays folded.
    expect(screen.getAllByText('2 keys')).toHaveLength(1);
  });

  it('steps through matches, wrapping, and marks the current one', async () => {
    const { component } = mount({ query: 'gpt' });
    const current = () => document.querySelectorAll('.shadow-\\[inset_2px_0_0_var\\(--warning\\)\\]');

    expect(current()).toHaveLength(0);

    component.step(1);
    await Promise.resolve();
    await Promise.resolve();
    expect(current()).toHaveLength(1);
    expect(current()[0]).toBe(lineOf('plan'));

    component.step(1);
    await Promise.resolve();
    await Promise.resolve();
    expect(current()[0]).toBe(lineOf('review'));

    component.step(1);
    await Promise.resolve();
    await Promise.resolve();
    expect(current()[0]).toBe(lineOf('plan'));

    component.step(-1);
    await Promise.resolve();
    await Promise.resolve();
    expect(current()[0]).toBe(lineOf('review'));
  });

  it('draws a tick on the minimap for each match', () => {
    const { container } = mount({ query: 'gpt' });

    expect(container.querySelectorAll('i.bg-warning')).toHaveLength(2);
  });

  it('has nothing to step through without a match', () => {
    const { component, container } = mount({ query: 'nothing' });

    component.step(1);

    expect(container.querySelectorAll('i.bg-warning')).toHaveLength(0);
  });

  it('says so when the file is empty', () => {
    mount({ fields: [] });

    expect(screen.getByRole('status')).toHaveTextContent('No entries in this file.');
  });

  it('renders only the rows in view for a long file', () => {
    const many = Array.from({ length: 450 }, (_, index) => field(`key${index}`, text('v')));
    mount({ fields: many });

    expect(screen.getByRole('list', { name: 'Source' })).toBeInTheDocument();
    expect(screen.queryAllByRole('listitem').length).toBeLessThan(450);
  });
});
