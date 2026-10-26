<script lang="ts">
  import ArrowDownToLine from '@lucide/svelte/icons/arrow-down-to-line';
  import ArrowUpFromLine from '@lucide/svelte/icons/arrow-up-from-line';
  import Cloud from '@lucide/svelte/icons/cloud';
  import Upload from '@lucide/svelte/icons/upload';

  import Button from '$lib/components/Button.svelte';
  import type { IconComponent } from '$lib/components/icon';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { BranchSync } from '$lib/ipc/bindings/BranchSync';
  import type { SyncAction } from '$lib/ipc/bindings/SyncAction';
  import { branchSync } from '$lib/stores/branch-sync.svelte';

  import SyncRow from './SyncRow.svelte';

  interface Props {
    sync: BranchSync;
    /** The remote the branch syncs with. */
    remote: string;
  }

  let { sync, remote }: Props = $props();

  interface Spec {
    title: string;
    subtitle: string;
    action: SyncAction;
    label: string;
    icon: IconComponent;
    disabled: boolean;
    /** Why the button is off, when it is. */
    reason?: string;
  }

  // How the branch stands against its remote decides the row; all of them keep the same shape.
  const spec = $derived.by((): Spec => {
    if (sync.upstream === null) {
      return { title: remote, subtitle: t('branch.remote.unpublished'), action: 'publish', label: t('branch.remote.publish'), icon: Upload, disabled: false };
    }

    const title = sync.upstream;

    if (sync.ahead > 0 && sync.behind > 0) {
      const subtitle = t('branch.remote.diverged', { ahead: sync.ahead, behind: sync.behind });

      return { title, subtitle, action: 'pull', label: t('branch.remote.pull'), icon: ArrowDownToLine, disabled: true, reason: t('branch.remote.pullBlocked') };
    }

    if (sync.behind > 0) {
      return { title, subtitle: tn('branch.remote.behind', sync.behind), action: 'pull', label: t('branch.remote.pull'), icon: ArrowDownToLine, disabled: false };
    }

    if (sync.ahead > 0) {
      return { title, subtitle: tn('branch.remote.ahead', sync.ahead), action: 'push', label: t('branch.remote.pushCount', { count: sync.ahead }), icon: ArrowUpFromLine, disabled: false };
    }

    return { title, subtitle: t('branch.remote.inSync'), action: 'push', label: t('branch.remote.push'), icon: ArrowUpFromLine, disabled: true };
  });
</script>

<SyncRow icon={Cloud} subtitle={spec.subtitle}>
  {#snippet title()}{spec.title}{/snippet}

  {#snippet action()}
    <Button title={spec.reason} disabled={spec.disabled || branchSync.busy !== null} loading={branchSync.busy === spec.action} onclick={() => void branchSync.run(spec.action)}>
      {#snippet leading()}{@const Icon = spec.icon}<Icon size={16} aria-hidden="true" />{/snippet}
      {spec.label}
    </Button>
  {/snippet}
</SyncRow>
