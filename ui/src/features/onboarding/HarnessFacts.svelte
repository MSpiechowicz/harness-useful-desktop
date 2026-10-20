<script lang="ts">
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';
  import { formatCount, relativeTime } from '$lib/utils/format';

  import type { ExtensionsFact, MachineFacts } from './machine-facts.svelte';

  interface Props {
    summary: HarnessSummary;
    /** What was read about the harness; undefined before the first read starts. */
    facts: MachineFacts | undefined;
    id: string;
  }

  let { summary, facts, id }: Props = $props();

  interface Tile {
    key: string;
    state: 'loading' | 'ready';
    value: string;
    label: string;
  }

  /** The harness's own nouns for what is installed in it, with their counts: plugins or packages, then MCP servers. */
  function installed(fact: ExtensionsFact): { value: string; label: string } {
    const counts: [number | null, string][] = [
      [fact.plugins, t('onboarding.facts.plugins')],
      [fact.packages, t('onboarding.facts.packages')],
      [fact.mcpServers, t('onboarding.facts.mcpServers')],
    ];
    const known = counts.filter((count): count is [number, string] => count[0] !== null);

    return { value: known.map(([count]) => formatCount(count)).join(' · '), label: known.map(([, label]) => label).join(' · ') };
  }

  /** Only what is known or on its way: a fact that could not be read has no tile, so the others share the row. */
  const tiles = $derived.by<Tile[]>(() => {
    const sessions = facts?.sessions ?? { status: 'loading' as const };
    const extensions = facts?.extensions ?? { status: 'loading' as const };
    const result: Tile[] = [];

    if (sessions.status === 'loading') {
      result.push({ key: 'sessions', state: 'loading', value: '', label: '' }, { key: 'last', state: 'loading', value: '', label: '' });
    }

    if (sessions.status === 'ready') {
      const { count, capped, lastUsed, project } = sessions.value;
      const label = project === null ? t('onboarding.facts.sessions') : t('onboarding.facts.sessionsIn', { project });

      result.push({ key: 'sessions', state: 'ready', value: capped ? `${formatCount(count)}+` : formatCount(count), label });

      if (lastUsed !== null) {
        result.push({ key: 'last', state: 'ready', value: relativeTime(lastUsed), label: t('onboarding.facts.lastUsed') });
      }
    }

    if (extensions.status === 'loading') {
      result.push({ key: 'extensions', state: 'loading', value: '', label: '' });
    }

    if (extensions.status === 'ready') {
      result.push({ key: 'extensions', state: 'ready', ...installed(extensions.value) });
    }

    return result;
  });

  const name = $derived(summary.info.name);
</script>

<!-- Nothing at all while no fact is known, so the panel never shows an empty heading. -->
{#if tiles.length > 0}
  <section class="flex flex-col gap-2" aria-labelledby={id}>
    <h4 {id} class="text-xs font-medium tracking-wide text-fg-3 uppercase">{t('onboarding.facts.title')}</h4>

    <ul class="flex gap-2" aria-label={t('onboarding.facts.list', { name })}>
      {#each tiles as tile (tile.key)}
        <li class="flex h-14 min-w-0 flex-1 basis-0 flex-col justify-center gap-0.5 rounded-md border border-line bg-surface-1 px-3">
          {#if tile.state === 'loading'}
            <Skeleton height="1.25rem" width="55%" />
            <Skeleton height="0.75rem" width="80%" />
          {:else}
            <span class="tabular truncate text-lg font-medium text-fg" title={tile.value}>{tile.value}</span>
            <span class="truncate text-xs text-fg-3" title={tile.label}>{tile.label}</span>
          {/if}
        </li>
      {/each}
    </ul>
  </section>
{/if}
