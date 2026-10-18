<script lang="ts">
  import ShieldCheck from '@lucide/svelte/icons/shield-check';

  import SectionCard from '$lib/components/SectionCard.svelte';
  import StatGrid from '$lib/components/StatGrid.svelte';
  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { ConfigSection } from '$lib/ipc/bindings/ConfigSection';
  import { formatCount } from '$lib/utils/format';

  import ProfilesCard from './ProfilesCard.svelte';
  import { shortPath } from './path';
  import { FORMAT_LABELS, LAYER_TITLES } from './sections';
  import { configStats } from './stats';

  interface Props {
    section: ConfigSection;
    profiles: { names: string[]; hint: string | null };
  }

  let { section, profiles }: Props = $props();

  const NONE = '—';

  // The same cards in the same order whatever the file holds; a file that cannot be read reads as dashes.
  const readable = $derived(section.source.exists && !section.error);
  const stats = $derived(readable ? configStats(section.entries) : null);

  const overview = $derived([
    { label: t('config.overview.entries'), value: stats ? formatCount(stats.entries) : NONE },
    { label: t('config.overview.values'), value: stats ? formatCount(stats.values) : NONE },
    { label: t('config.overview.depth'), value: stats ? formatCount(stats.depth) : NONE },
    { label: t('config.overview.secrets'), value: stats ? formatCount(stats.secrets) : NONE },
  ]);

  const file = $derived([
    { label: t('config.file.layer'), value: LAYER_TITLES[section.source.layer] },
    { label: t('config.file.format'), value: FORMAT_LABELS[section.source.format] },
    ...(section.source.profile ? [{ label: t('config.file.profile'), value: section.source.profile }] : []),
  ]);
</script>

<!-- The body of the details panel, which supplies the heading, the padding and the scrolling. -->
<SectionCard flat heading={t('config.overview.title')}>
  <StatGrid stats={overview} />
</SectionCard>

<SectionCard flat heading={t('config.file.title')}>
  <dl class="flex flex-col">
    {#each file as row (row.label)}
      <div class="flex justify-between gap-3 py-1 text-sm">
        <dt class="text-fg-2">{row.label}</dt>
        <dd class="min-w-0 truncate text-right text-fg" title={row.value}>{row.value}</dd>
      </div>
    {/each}

    <!-- Same type as the rows above, with the path shortened (~ and the last folders); the tooltip has all of it. Copying is in the top bar's menu. -->
    <div class="flex justify-between gap-3 py-1 text-sm">
      <dt class="shrink-0 text-fg-2">{t('config.file.path')}</dt>
      <dd class="min-w-0 truncate text-right text-fg" title={section.source.path}>{shortPath(section.source.path)}</dd>
    </div>
  </dl>

  <p class="mt-2 flex items-start gap-2 text-xs text-fg-3">
    <ShieldCheck size={ICON.sm} class="mt-px shrink-0 text-success" aria-hidden="true" />
    {t('config.detail.secrets')}
  </p>
</SectionCard>

{#if profiles.names.length > 0}
  <ProfilesCard names={profiles.names} hint={profiles.hint} />
{/if}
