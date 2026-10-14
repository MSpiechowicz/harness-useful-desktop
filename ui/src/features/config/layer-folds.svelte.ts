import type { ConfigLayer } from '$lib/ipc/bindings/ConfigLayer';
import { Folded } from '$lib/utils/folded.svelte';

/** The layers the user folded away in the file list; kept outside the view so the choice survives a visit to another view. */
export const folded = new Folded<ConfigLayer>();
