import type { MemorySavePlace } from '$lib/ipc/bindings/MemorySavePlace';

import type { SavePlace } from '../memory/save-place.svelte';

/** The Save to field of every approval card on screen, by held id: Save all saves each item with its own card's choice. */
const fields = new Map<string, SavePlace>();

export const heldPlaces = {
  /** Registers the field of a card. Returns the function that forgets it when the card goes. */
  register(heldId: string, place: SavePlace): () => void {
    fields.set(heldId, place);

    return () => {
      if (fields.get(heldId) === place) {
        fields.delete(heldId);
      }
    };
  },

  /** The place a save of this held item names: what its card's field says while the field shows, nothing otherwise. */
  sent(heldId: string): MemorySavePlace | undefined {
    return fields.get(heldId)?.sent;
  },
};
