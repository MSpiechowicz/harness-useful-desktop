import type { SessionNode } from '$lib/ipc/bindings/SessionNode';

/** A linear run at least this long is folded into one "N more messages" row. */
export const MIN_FOLD = 5;

const FOLD_PREFIX = 'fold:';

/** One line of the vertical graph: a message dot or a folded run. */
export interface GraphRow {
  /** The node id, or `fold:<first id>` for a folded run. */
  key: string;
  node: SessionNode | null;
  /** Ids of the folded nodes; empty for a node row. */
  folded: string[];
  lane: number;
  active: boolean;
  /** The node has more than one child. */
  branchPoint: boolean;
  /** Lane the connector entering this row from above starts in; null for a root. */
  from: number | null;
  /** The connector toward the next row of the same lane leaves the dot downward. */
  down: boolean;
  /** Lanes whose line runs straight through this row (a branch's trunk passing a side branch). */
  through: { lane: number; active: boolean }[];
}

export interface TreeLayout {
  rows: GraphRow[];
  lanes: number;
}

export interface LayoutOptions {
  /** Folded runs the user opened, by `fold:<first id>`. */
  expanded: ReadonlySet<string>;
  /** Nodes that must stay visible (the selection). */
  keep: ReadonlySet<string>;
}

interface VNode {
  id: string;
  parentId: string | null;
  node: SessionNode | null;
  folded: string[];
  active: boolean;
  children: string[];
}

export function isFoldKey(key: string): boolean {
  return key.startsWith(FOLD_PREFIX);
}

/** Linear runs of foldable nodes: a single child, a single-child parent, no label, not kept. */
function findRuns(nodes: SessionNode[], children: Map<string, string[]>, options: LayoutOptions): string[][] {
  const byId = new Map(nodes.map((node) => [node.id, node]));

  const foldable = (node: SessionNode): boolean => {
    const parent = node.parentId === null ? undefined : byId.get(node.parentId);

    return (
      parent !== undefined &&
      (children.get(parent.id)?.length ?? 0) === 1 &&
      (children.get(node.id)?.length ?? 0) === 1 &&
      node.label === null &&
      !options.keep.has(node.id)
    );
  };

  const runs: string[][] = [];

  for (const node of nodes) {
    const parent = node.parentId === null ? undefined : byId.get(node.parentId);
    if (!foldable(node) || (parent && foldable(parent))) {
      continue;
    }

    // `node` starts a run: follow the chain while it stays foldable.
    const run: string[] = [];
    let current: SessionNode | undefined = node;

    while (current && foldable(current)) {
      run.push(current.id);
      current = byId.get(children.get(current.id)?.[0] ?? '');
    }

    if (run.length >= MIN_FOLD && !options.expanded.has(`${FOLD_PREFIX}${run[0]}`)) {
      runs.push(run);
    }
  }

  return runs;
}

/** The tree as virtual nodes, with each qualifying run replaced by one fold node. */
function buildVirtualNodes(nodes: SessionNode[], options: LayoutOptions): Map<string, VNode> {
  const known = new Set(nodes.map((node) => node.id));
  const rawChildren = new Map<string, string[]>();

  for (const node of nodes) {
    if (node.parentId !== null && known.has(node.parentId)) {
      rawChildren.set(node.parentId, [...(rawChildren.get(node.parentId) ?? []), node.id]);
    }
  }

  const runs = findRuns(nodes, rawChildren, options);
  const foldOf = new Map<string, string>();
  const runByFold = new Map<string, string[]>();
  for (const run of runs) {
    runByFold.set(`${FOLD_PREFIX}${run[0]}`, run);
    run.forEach((id) => foldOf.set(id, `${FOLD_PREFIX}${run[0]}`));
  }

  const virtual = new Map<string, VNode>();
  const byId = new Map(nodes.map((node) => [node.id, node]));

  for (const node of nodes) {
    const fold = foldOf.get(node.id);

    if (fold === undefined) {
      const parentId = node.parentId !== null && known.has(node.parentId) ? (foldOf.get(node.parentId) ?? node.parentId) : null;
      virtual.set(node.id, { id: node.id, parentId, node, folded: [], active: node.onActivePath, children: [] });
    } else if (!virtual.has(fold)) {
      const run = runByFold.get(fold) ?? [];
      const first = byId.get(run[0]);
      virtual.set(fold, { id: fold, parentId: first?.parentId ?? null, node: null, folded: run, active: first?.onActivePath ?? false, children: [] });
    }
  }

  for (const entry of virtual.values()) {
    if (entry.parentId !== null && virtual.has(entry.parentId)) {
      virtual.get(entry.parentId)?.children.push(entry.id);
    } else {
      entry.parentId = null;
    }
  }

  return virtual;
}

/** The child that continues its parent's lane: the one on the active path, else the latest. */
function mainChild(children: string[], virtual: Map<string, VNode>): string {
  return children.find((id) => virtual.get(id)?.active) ?? children[children.length - 1];
}

/**
 * Lays the session tree out as a vertical graph, newest at the bottom. At a branch point the
 * side branches are drawn first on lanes to the right, then the main line carries on in the
 * parent's lane, so the active path stays a straight spine. Iterative, as sessions can be deep.
 */
export function layoutTree(nodes: SessionNode[], options: LayoutOptions): TreeLayout {
  const virtual = buildVirtualNodes(nodes, options);
  const rows: GraphRow[] = [];
  const stack: { id: string; lane: number; parent: number | null }[] = [];

  const roots = [...virtual.values()].filter((entry) => entry.parentId === null);
  for (const root of roots.reverse()) {
    stack.push({ id: root.id, lane: 0, parent: null });
  }

  let lanes = 1;

  while (stack.length > 0) {
    const frame = stack.pop();
    const entry = frame && virtual.get(frame.id);
    if (!frame || !entry) {
      continue;
    }

    const index = rows.length;
    const parent = frame.parent === null ? null : rows[frame.parent];
    const row: GraphRow = {
      key: entry.id,
      node: entry.node,
      folded: entry.folded,
      lane: frame.lane,
      active: entry.active,
      branchPoint: entry.children.length > 1,
      from: parent ? parent.lane : null,
      down: false,
      through: [],
    };

    rows.push(row);
    lanes = Math.max(lanes, frame.lane + 1);

    // A child in the parent's own lane: the trunk runs straight through whatever side branches sit between.
    if (parent && frame.parent !== null && parent.lane === frame.lane) {
      parent.down = true;

      for (let between = frame.parent + 1; between < index; between += 1) {
        rows[between].through.push({ lane: frame.lane, active: entry.active });
      }
    }

    if (entry.children.length > 0) {
      const main = mainChild(entry.children, virtual);
      stack.push({ id: main, lane: frame.lane, parent: index });

      // Side branches come off first, in file order, so they are pushed last.
      for (const side of entry.children.filter((id) => id !== main).reverse()) {
        stack.push({ id: side, lane: frame.lane + 1, parent: index });
      }
    }
  }

  return { rows, lanes };
}
