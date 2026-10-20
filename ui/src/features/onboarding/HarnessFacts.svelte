<script lang="ts">
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { DURATION, rise } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';
  import { cx } from '$lib/utils/cx';
  import { formatCount, shortRelativeTime } from '$lib/utils/format';

  import { FACTS } from './layout';
  import type { MachineFacts } from './machine-facts.svelte';

  interface Props {
    summary: HarnessSummary;
    /** What was read about the harness; undefined before the first read starts. */
    facts: MachineFacts | undefined;
    id: string;
  }

  let { summary, facts, id }: Props = $props();

  interface Tile {
    key: string;
    label: string;
    /** The number, or null while it is being read (and, once all are read, for a fact that is not known). */
    value: string | null;
    /** Whether this fact can turn up for this harness, so its place is kept while the facts are being read. */
    expected: boolean;
    /** What the label alone leaves out (the project a count is for), as a tooltip and for assistive technology. */
    note?: string;
  }

  const sessions = $derived(facts?.sessions ?? { status: 'loading' as const });
  const extensions = $derived(facts?.extensions ?? { status: 'loading' as const });

  /** Whether every fact is either known or given up on, so the tiles are laid out once and not as each answer comes in. */
  const settled = $derived(sessions.status !== 'loading' && extensions.status !== 'loading');

  const countOf = (count: number | null | undefined): string | null => (count == null ? null : formatCount(count));

  /** Every tile the harness could have, one number each. The labels are known up front, so only the values wait. */
  const candidates = $derived.by<Tile[]>(() => {
    const read = sessions.status === 'ready' ? sessions.value : null;
    const installed = extensions.status === 'ready' ? extensions.value : null;
    const { capabilities } = summary;

    return [
      {
        key: 'sessions',
        label: t('onboarding.facts.sessions'),
        value: read ? `${formatCount(read.count)}${read.capped ? '+' : ''}` : null,
        expected: true,
        note: read?.project ? t('onboarding.facts.sessionsIn', { project: read.project }) : undefined,
      },
      { key: 'last', label: t('onboarding.facts.lastUsed'), value: read?.lastUsed ? shortRelativeTime(read.lastUsed) : null, expected: true },
      { key: 'plugins', label: t('onboarding.facts.plugins'), value: countOf(installed?.plugins), expected: capabilities.plugins },
      { key: 'packages', label: t('onboarding.facts.packages'), value: countOf(installed?.packages), expected: capabilities.packages },
      { key: 'mcpServers', label: t('onboarding.facts.mcpServers'), value: countOf(installed?.mcpServers), expected: capabilities.mcp },
    ];
  });

  /**
   * While any fact is still being read, every tile that might turn up has its place (with a placeholder for the value).
   * Once all are settled the ones that could not be read are gone and the others share the row, so the row changes once
   * and not as each answer comes in.
   */
  const tiles = $derived(candidates.filter((tile) => (settled ? tile.value !== null : tile.expected)));

  const name = $derived(summary.info.name);
</script>

<!-- Nothing at all while no fact is known, so the panel never shows an empty heading. -->
{#if tiles.length > 0}
  <section class={cx('flex flex-col gap-2', FACTS.section)} aria-labelledby={id}>
    <h4 {id} class="text-xs font-medium tracking-wide text-fg-3 uppercase">{t('onboarding.facts.title')}</h4>

    <ul class={FACTS.row} aria-label={t('onboarding.facts.list', { name })}>
      {#each tiles as tile (tile.key)}
        <li class={FACTS.tile} title={tile.note} aria-describedby={tile.note ? `${id}-${tile.key}` : undefined}>
          <span class={FACTS.value}>
            {#if !settled || tile.value === null}
              <Skeleton height="1.125rem" width="3.5rem" />
            {:else}
              <span class="tabular text-lg font-medium text-fg" in:rise={{ y: 4, duration: DURATION.base }}>{tile.value}</span>
            {/if}
          </span>

          <span class={FACTS.label}>{tile.label}</span>

          {#if tile.note}
            <span hidden id="{id}-{tile.key}">{tile.note}</span>
          {/if}
        </li>
      {/each}
    </ul>
  </section>
{/if}
