import { describe, expect, it } from 'vitest';

import type { SessionNode } from '$lib/ipc/bindings/SessionNode';
import { fixtures } from '$lib/ipc/mock/fixtures';

import { isFoldKey, layoutTree, MIN_FOLD, type LayoutOptions } from './tree-layout';

const OPEN: LayoutOptions = { expanded: new Set(), keep: new Set() };

function node(id: string, parentId: string | null, over: Partial<SessionNode> = {}): SessionNode {
  return { id, parentId, kind: 'user', preview: id, timestamp: null, label: null, onActivePath: true, childCount: 0, ...over };
}

/** A straight chain n0 -> n1 -> ... with the child counts a real tree would report. */
function chain(length: number, prefix = 'n'): SessionNode[] {
  return Array.from({ length }, (_, index) => node(`${prefix}${index}`, index === 0 ? null : `${prefix}${index - 1}`, { childCount: index === length - 1 ? 0 : 1 }));
}

describe('layout of the sample session', () => {
  const tree = fixtures.sessions.tree;
  const layout = layoutTree(tree.nodes, OPEN);

  it('draws every entry once, newest last, with branch points marked', () => {
    const nodeRows = layout.rows.filter((row) => row.node !== null);

    expect(nodeRows).toHaveLength(tree.nodes.length);
    expect(nodeRows.filter((row) => row.branchPoint)).toHaveLength(tree.branchCount);
    expect(nodeRows.at(-1)?.node?.id).toBe(tree.leafId);
  });

  it('keeps the active path as one straight spine on the first lane', () => {
    const active = layout.rows.filter((row) => row.node?.onActivePath);

    expect(active.length).toBeGreaterThan(0);
    expect(active.every((row) => row.lane === 0 && row.active)).toBe(true);
  });

  it('puts side branches on lanes to the right, before the main line carries on', () => {
    expect(layout.lanes).toBeGreaterThan(1);

    const rows = layout.rows;
    for (const [index, row] of rows.entries()) {
      if (row.branchPoint && row.node?.onActivePath) {
        const next = rows.findIndex((candidate, at) => at > index && candidate.lane === 0 && candidate.node?.parentId === row.node?.id);
        const sides = rows.slice(index + 1, next).filter((candidate) => candidate.lane > 0);

        expect(sides.length).toBeGreaterThan(0);
        expect(rows[index + 1].lane).toBe(1);
        expect(rows[index + 1].from).toBe(0);
        expect(rows[next].active).toBe(true);
      }
    }
  });

  it('runs the trunk through the rows of a side branch', () => {
    const throughs = layout.rows.filter((row) => row.lane > 0).flatMap((row) => row.through);

    expect(throughs.some((line) => line.lane === 0 && line.active)).toBe(true);
  });
});

describe('folding long linear runs', () => {
  const nodes = chain(MIN_FOLD + 7);

  it('replaces a long run with one "N more messages" row', () => {
    const { rows } = layoutTree(nodes, OPEN);
    const fold = rows.find((row) => isFoldKey(row.key));

    // The first and last entries stay visible: the first has no parent, the last no child.
    expect(rows).toHaveLength(3);
    expect(fold?.folded).toHaveLength(nodes.length - 2);
    expect(rows[0].node?.id).toBe('n0');
    expect(rows.at(-1)?.node?.id).toBe(`n${nodes.length - 1}`);
  });

  it('shows the whole run once it is opened', () => {
    const { rows } = layoutTree(nodes, { expanded: new Set(['fold:n1']), keep: new Set() });

    expect(rows).toHaveLength(nodes.length);
    expect(rows.some((row) => isFoldKey(row.key))).toBe(false);
  });

  it('never folds the selection or a labelled entry', () => {
    const labelled = nodes.map((entry) => (entry.id === 'n4' ? { ...entry, label: 'checkpoint' } : entry));

    const { rows } = layoutTree(labelled, { expanded: new Set(), keep: new Set(['n8']) });
    const visible = rows.flatMap((row) => (row.node ? [row.node.id] : []));

    expect(visible).toContain('n4');
    expect(visible).toContain('n8');
  });

  it('leaves short runs and branch starts alone', () => {
    const short = layoutTree(chain(MIN_FOLD - 1), OPEN).rows;

    expect(short.some((row) => isFoldKey(row.key))).toBe(false);

    const forked = [node('root', null, { childCount: 2 }), node('a', 'root', { childCount: 0 }), node('b', 'root', { onActivePath: false, childCount: 0 })];

    expect(layoutTree(forked, OPEN).rows).toHaveLength(3);
  });

  it('copes with a very deep session without recursing', () => {
    const { rows } = layoutTree(chain(25_000), OPEN);

    expect(rows.length).toBeLessThan(10);
  });
});
