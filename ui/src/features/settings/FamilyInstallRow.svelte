<script lang="ts">
  import Download from '@lucide/svelte/icons/download';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';

  import Button from '$lib/components/Button.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { nav } from '$lib/stores/nav.svelte';

  import FamilyInstallConfirm from './FamilyInstallConfirm.svelte';
  import { FamilyInstallFlow, type InstallMember } from './family-install.svelte';
  import SettingRow from './SettingRow.svelte';

  interface Props {
    member: InstallMember;
    /** The member is installed but too old for what the app needs. Otherwise it is not installed at all. */
    update: boolean;
    /** Looks at the installed member again once the installer ran. */
    onfinished: () => Promise<void>;
  }

  let { member, update, onfinished }: Props = $props();

  // svelte-ignore state_referenced_locally
  const flow = new FamilyInstallFlow(
    member,
    () => update,
    () => onfinished(),
  );

  const copy = $derived(flow.copy);
  const Icon = $derived(update ? RefreshCw : Download);
  const label = $derived(t(update ? copy.update : copy.install));
</script>

<SettingRow title={t(update ? copy.outdated.title : copy.missing.title)} description={t(update ? copy.outdated.description : copy.missing.description)}>
  {#snippet control()}
    <div class="flex items-center gap-2">
      <Button variant="ghost" onclick={() => nav.go('ecosystem')}>{t(copy.ecosystem)}</Button>
      <Button variant="primary" loading={flow.busy} onclick={() => void flow.review()}>
        {#snippet leading()}
          <Icon size={16} aria-hidden="true" />
        {/snippet}
        {flow.phase === 'running' ? t(update ? copy.updating : copy.installing) : label}
      </Button>
    </div>
  {/snippet}
  {#snippet below()}
    {#if flow.phase === 'failed'}
      <p class="pt-2 text-sm text-danger" role="alert">{flow.failure}</p>
    {/if}
  {/snippet}
</SettingRow>

<FamilyInstallConfirm {flow} />
