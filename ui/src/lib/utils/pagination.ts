/** A page button of the pager (zero-based page) or the ellipsis that stands for the pages left out. */
export type PageItem = number | 'gap';

/** How many pages a list of `total` entries makes; an empty list still has one (empty) page. */
export function pageCount(total: number, pageSize: number): number {
  return Math.max(1, Math.ceil(total / pageSize));
}

/** The page that exists nearest to `page`: a list that shrank (a delete, a narrower search) never leaves the pager past its end. */
export function clampPage(page: number, total: number, pageSize: number): number {
  return Math.min(Math.max(0, page), pageCount(total, pageSize) - 1);
}

/**
 * The page buttons to show: the first and the last page, the current one with a neighbour on each side, and an ellipsis
 * where pages are left out. Near either end four pages are shown together, so the pager keeps the same width while paging.
 */
export function pageItems(current: number, pages: number): PageItem[] {
  const last = pages - 1;
  const shown = new Set<number>([0, last, current - 1, current, current + 1]);

  if (current <= 2) {
    [0, 1, 2, 3].forEach((page) => shown.add(page));
  }

  if (current >= last - 2) {
    [last - 3, last - 2, last - 1, last].forEach((page) => shown.add(page));
  }

  const sorted = [...shown].filter((page) => page >= 0 && page <= last).sort((a, b) => a - b);
  const items: PageItem[] = [];

  sorted.forEach((page, index) => {
    const before = sorted[index - 1];

    if (before !== undefined && page - before === 2) {
      items.push(before + 1);
    } else if (before !== undefined && page - before > 2) {
      items.push('gap');
    }

    items.push(page);
  });

  return items;
}
