<script lang="ts">
  import ClipboardCopy from '@lucide/svelte/icons/clipboard-copy';
  import ExternalLink from '@lucide/svelte/icons/external-link';
  import Trash from '@lucide/svelte/icons/trash-2';

  import Button from '$lib/components/Button.svelte';
  import { openExternal } from '$lib/ipc/actions';
  import { app } from '$lib/stores/app.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';

  import { diagnosticsText } from './diagnostics';
  import { PRIVACY_FACTS } from './privacy-facts';
  import SettingRow from './SettingRow.svelte';
  import SettingsSection from './SettingsSection.svelte';

  const REPOSITORY_URL = 'https://github.com/MSpiechowicz/harness-useful-desktop';

  async function clearRecents(): Promise<void> {
    for (const recent of [...project.recents]) {
      await project.forget(recent.path);
    }

    if (project.recents.length === 0) {
      toasts.push({ title: 'Recent projects cleared', tone: 'success' });
    }
  }

  async function copyDiagnostics(): Promise<void> {
    try {
      await navigator.clipboard.writeText(diagnosticsText(app.info, harness.summaries));
    } catch {
      toasts.push({ title: 'Could not copy', description: 'The clipboard is not available.', tone: 'warning' });

      return;
    }

    toasts.push({ title: 'Diagnostics copied', description: 'App version, OS and harness versions only. No paths or content.', tone: 'success' });
  }
</script>

<SettingsSection title="Privacy & security">
  <ul class="flex flex-col gap-4 pb-4" aria-label="What Harness Desktop does with data">
    {#each PRIVACY_FACTS as fact (fact.id)}
      <li class="flex items-start gap-3">
        <span class="mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-md bg-surface-2 text-fg-2">
          <fact.icon size={15} aria-hidden="true" />
        </span>

        <div class="flex min-w-0 flex-col gap-0.5">
          <p class="text-base font-medium text-fg">{fact.title}</p>
          <p class="text-sm text-fg-2">{fact.text}</p>
        </div>
      </li>
    {/each}
  </ul>

  <SettingRow title="Recent projects" description="Forget every remembered folder. If your open project is on the list, it is closed too.">
    {#snippet control()}
      <Button disabled={project.recents.length === 0} onclick={() => void clearRecents()}>
        {#snippet leading()}<Trash size={14} aria-hidden="true" />{/snippet}
        Clear recent projects
      </Button>
    {/snippet}
  </SettingRow>

  <SettingRow title="Diagnostics" description="Copies the app version, OS and harness versions for a bug report. No paths, projects or content.">
    {#snippet control()}
      <Button onclick={() => void copyDiagnostics()}>
        {#snippet leading()}<ClipboardCopy size={14} aria-hidden="true" />{/snippet}
        Copy diagnostics
      </Button>
    {/snippet}
  </SettingRow>

  <SettingRow title="Source code" description="Free software under GPL-3.0-or-later. Read exactly what it does.">
    {#snippet control()}
      <Button onclick={() => void openExternal(REPOSITORY_URL)}>
        {#snippet leading()}<ExternalLink size={14} aria-hidden="true" />{/snippet}
        Open repository
      </Button>
    {/snippet}
  </SettingRow>
</SettingsSection>
