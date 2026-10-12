<script lang="ts">
  import ExternalLink from '@lucide/svelte/icons/external-link';
  import ShieldCheck from '@lucide/svelte/icons/shield-check';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import { t } from '$lib/i18n/i18n.svelte';
  import Logo from '$lib/components/Logo.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { openExternal } from '$lib/ipc/actions';
  import { app } from '$lib/stores/app.svelte';

  import { LICENSE_URL, RELEASES_URL, REPOSITORY_URL } from './links';

  const LINKS = [
    { label: () => t('settings.about.source'), url: REPOSITORY_URL },
    { label: () => t('settings.about.releases'), url: RELEASES_URL },
    { label: () => t('settings.about.licence'), url: LICENSE_URL },
  ];
</script>

<section aria-label={t('settings.about.label')} class="flex flex-col gap-4 rounded-lg border border-line bg-surface-1 p-5 shadow-raised">
  <!-- The badge sits on the title line, like the controls of a settings row. -->
  <div class="flex items-start gap-5">
    <Logo size={44} class="self-center" />

    <div class="flex min-w-0 flex-1 flex-col gap-1">
      {#if app.info}
        <h2 class="text-xl text-fg">{app.info.name}</h2>
        <p class="tabular text-sm text-fg-2">{t(app.info.debug ? 'settings.about.versionDebug' : 'settings.about.version', { version: app.info.version, os: app.info.os, arch: app.info.arch })}</p>
      {:else}
        <Skeleton width="10rem" height="1.5rem" />
        <Skeleton width="14rem" height="0.875rem" />
      {/if}
    </div>

    <Badge tone="success" class="mt-1 px-3 py-1"><ShieldCheck size={14} aria-hidden="true" />{t('settings.about.localOnly')}</Badge>
  </div>

  <ul class="flex flex-wrap gap-2 border-t border-line pt-4" aria-label={t('settings.about.links')}>
    {#each LINKS as link (link.url)}
      <li>
        <Button variant="ghost" size="sm" onclick={() => void openExternal(link.url)}>
          {#snippet leading()}<ExternalLink size={ICON.sm} aria-hidden="true" />{/snippet}
          {link.label()}
        </Button>
      </li>
    {/each}
  </ul>
</section>
