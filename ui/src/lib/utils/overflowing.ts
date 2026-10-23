import type { Attachment } from 'svelte/attachments';

/**
 * An attachment that tells `report` whether the element's content is taller than the element: when it attaches and whenever the
 * element changes size. `content` is what the element shows, so a change of it measures again (the element's own size may stay).
 * A region that scrolls only then needs to be a tab stop.
 */
export function overflowing(content: unknown, report: (overflows: boolean) => void): Attachment<HTMLElement> {
  void content;

  return (element) => {
    const measure = () => report(element.scrollHeight > element.clientHeight);
    measure();

    if (typeof ResizeObserver === 'undefined') {
      return;
    }

    const observer = new ResizeObserver(measure);
    observer.observe(element);

    return () => observer.disconnect();
  };
}
