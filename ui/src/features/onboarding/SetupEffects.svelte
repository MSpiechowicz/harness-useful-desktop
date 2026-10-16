<script lang="ts">
  import AppWindow from '@lucide/svelte/icons/app-window';
  import FolderDown from '@lucide/svelte/icons/folder-down';
  import Info from '@lucide/svelte/icons/info';
  import Plug from '@lucide/svelte/icons/plug';
  import Square from '@lucide/svelte/icons/square';
  import Terminal from '@lucide/svelte/icons/terminal';

  import type { IconComponent } from '$lib/components/icon';

  import { describeEffect, pieces, type EffectKind } from './setup-effects';

  interface Props {
    /** What the installer changes besides its own files, as the backend words it. */
    effects: string[];
  }

  let { effects }: Props = $props();

  const ICONS: Record<EffectKind, IconComponent> = {
    install: FolderDown,
    menu: AppWindow,
    path: Terminal,
    register: Plug,
    stop: Square,
    other: Info,
  };

  const rows = $derived(effects.map(describeEffect));
</script>

{#snippet sentence(text: string)}
  {#each pieces(text) as piece, index (index)}
    {#if piece.code}<code class="rounded-sm bg-surface-3 px-[3px] py-px font-mono text-xs text-fg">{piece.text}</code>{:else}{piece.text}{/if}
  {/each}
{/snippet}

<ul class="flex flex-col gap-3">
  {#each rows as row, index (index)}
    {@const Icon = ICONS[row.kind]}
    <li class="flex items-center gap-3">
      <span class="grid size-8 shrink-0 place-items-center rounded-md bg-surface-3 text-fg-2" aria-hidden="true"><Icon size={16} /></span>

      <div class="max-w-[64ch] min-w-0">
        <p class="text-sm leading-normal text-fg">{@render sentence(row.title)}</p>
        {#if row.detail}
          <p class="text-xs leading-normal text-fg-3">{@render sentence(row.detail)}</p>
        {/if}
      </div>
    </li>
  {/each}
</ul>
