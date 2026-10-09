import type { ConfigView } from '$lib/ipc/bindings/ConfigView';
import type { ExtensionsView } from '$lib/ipc/bindings/ExtensionsView';
import { ipc } from '$lib/ipc/client';

import { ViewLoader } from './view-loader.svelte';

/** The readable configuration of the active harness and project. */
export const configView = new ViewLoader<ConfigView>((harness, project) => ipc.invoke('config_view', { harness, project }));

/** Plugins, skills, MCP servers and the rest installed into the active harness. */
export const extensionsView = new ViewLoader<ExtensionsView>((harness, project) => ipc.invoke('extensions_view', { harness, project }));
