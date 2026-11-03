import { pendingKey } from './pending-send';
import type { ChatState } from './types';

/**
 * Thumbnails of the images this window sent. The transcript keeps no image data, so each user message gets its thumbnails by
 * key: a message waiting for its echo under its own, the echo under its own once it arrives.
 */
export class SentImages {
  /** Thumbnails by message key. */
  byKey = $state.raw<Record<string, string[]>>({});
  /** Thumbnails of sent messages not yet matched to their transcript item, oldest first. */
  private unmatched: string[][] = [];
  /** Sends whose thumbnails are settled on their message already. */
  private readonly released = new Set<string>();

  /** A send with images is waiting for its message event: queued before the send, since the event may land before the reply does. */
  expect(thumbnails: string[]): void {
    if (thumbnails.length > 0) {
      this.unmatched.push(thumbnails);
    }
  }

  /** The send did not go out. */
  forget(thumbnails: string[]): void {
    this.unmatched = this.unmatched.filter((queued) => queued !== thumbnails);
  }

  /** Shows the thumbnails on a message that waits for its echo. */
  showWhileSending(id: string, thumbnails: string[]): void {
    if (thumbnails.length > 0) {
      this.byKey = { ...this.byKey, [pendingKey(id)]: thumbnails };
    }
  }

  /** Gives each new user message that carries images the thumbnails of the oldest send still waiting. History items (order -1) are older than any send and never match. */
  match(after: ChatState): void {
    if (this.unmatched.length === 0) {
      return;
    }

    const matched: Record<string, string[]> = {};

    for (const item of after.items) {
      const hasImages = item.kind === 'message' && item.role === 'user' && item.order >= 0 && item.blocks.some((block) => block.type === 'image');
      if (!hasImages || item.key in this.byKey || this.unmatched.length === 0) {
        continue;
      }

      matched[item.key] = this.unmatched.shift() ?? [];
    }

    if (Object.keys(matched).length > 0) {
      this.byKey = { ...this.byKey, ...matched };
    }
  }

  /**
   * A message that waited for its echo hands its thumbnails to the echo that replaced it (whether or not the echo lists image
   * blocks), then the thumbnails of messages no longer in the transcript are forgotten. A message that never got an echo keeps its own.
   */
  prune(after: ChatState): void {
    let byKey = this.byKey;

    for (const item of after.items) {
      if (item.kind !== 'message' || !item.send || item.send.status === 'sending') {
        continue;
      }

      const handed = byKey[pendingKey(item.send.id)];
      if (!handed || this.released.has(item.send.id)) {
        continue;
      }

      this.released.add(item.send.id);
      if (!(item.key in byKey)) {
        byKey = { ...byKey, [item.key]: handed };
      }

      // The same send's queued copy must not go to the next message with images.
      const same = this.unmatched.findIndex((queued) => queued.length === handed.length && queued.every((url, index) => url === handed[index]));
      if (same !== -1) {
        this.unmatched.splice(same, 1);
      }
    }

    const present = new Set(after.items.map((item) => item.key));
    const stale = Object.keys(byKey).filter((key) => key.startsWith(pendingKey('')) && !present.has(key));

    if (stale.length > 0) {
      byKey = Object.fromEntries(Object.entries(byKey).filter(([key]) => !stale.includes(key)));
    }

    if (byKey !== this.byKey) {
      this.byKey = byKey;
    }
  }
}
