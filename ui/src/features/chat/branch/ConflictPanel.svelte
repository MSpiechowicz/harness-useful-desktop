<script lang="ts">
  import FileDiff from '@lucide/svelte/icons/file-diff';
  import FileExclamationPoint from '@lucide/svelte/icons/file-exclamation-point';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import WandSparkles from '@lucide/svelte/icons/wand-sparkles';
  import X from '@lucide/svelte/icons/x';

  import Button from '$lib/components/Button.svelte';
  import StatusDot from '$lib/components/StatusDot.svelte';
  import type { ChatSession } from '$lib/chat/session.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import type { BranchSync } from '$lib/ipc/bindings/BranchSync';
  import type { GitOperation } from '$lib/ipc/bindings/GitOperation';
  import { branchSync } from '$lib/stores/branch-sync.svelte';
  import { git } from '$lib/stores/git.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { nav } from '$lib/stores/nav.svelte';
  import { toasts } from '$lib/stores/toasts.svelte';
  import { cx } from '$lib/utils/cx';

  import { conflictPrompt } from './conflict-prompt';
  import SyncRow from './SyncRow.svelte';

  interface Props {
    sync: BranchSync;
    /** The rebase or merge that stopped. Null for conflicts an autostash or stash pop left behind: then the files are `sync.unmerged`. */
    operation: GitOperation | null;
    session: ChatSession;
    /** Closes the branch menu. */
    close: () => void;
  }

  let { sync, operation, session, close }: Props = $props();

  const FILE = 'flex h-control w-full items-center gap-2 rounded-md px-2 text-left text-sm';

  const busy = $derived(branchSync.busy !== null);
  const harnessName = $derived(harness.get(session.info.harness)?.info.name ?? session.info.harness);
  const kind = $derived(operation?.kind ?? 'stash');
  const files = $derived(operation ? operation.conflicts : sync.unmerged);
  const onto = $derived(operation ? operation.onto : (sync.main?.name ?? null));
  const resolved = $derived(operation !== null && files.length === 0);
  const firstInProject = $derived(files.find((file) => file.inProject));

  const heading = $derived.by(() => {
    if (onto === null) {
      return kind === 'rebase' ? t('branch.conflict.rebaseAnon') : kind === 'merge' ? t('branch.conflict.mergeAnon') : t('branch.conflict.stashAnon');
    }

    return kind === 'rebase' ? t('branch.conflict.rebase', { main: onto }) : kind === 'merge' ? t('branch.conflict.merge', { main: onto }) : t('branch.conflict.stash', { main: onto });
  });

  /** Shows the file in the Changes view, which is where the conflict markers can be read. */
  function open(path: string): void {
    close();
    git.reveal(path);
    nav.go('changes');
  }

  /** Hands the conflicts to the harness of this chat, as the user's own message. */
  async function ask(): Promise<void> {
    const prompt = conflictPrompt({ kind, branch: sync.branch, onto, files });
    const sent = await session.submit(prompt, session.streaming ? 'followUp' : 'send');

    if (sent) {
      close();
    } else {
      toasts.push({ title: t('branch.conflict.askFailed', { harness: harnessName }), tone: 'warning' });
    }
  }
</script>

{#snippet label(name: string, folder: string)}
  <FileExclamationPoint size={16} aria-hidden="true" class="shrink-0 text-danger" />
  <span class="shrink-0 font-mono">{name}</span>
  {#if folder}<span class="min-w-0 truncate text-xs text-fg-3">{folder}</span>{/if}
{/snippet}

<div class="mx-2.5 mb-2.5 rounded-md border border-line bg-surface-2">
  <SyncRow icon={TriangleAlert} subtitle={resolved ? t('branch.conflict.resolved') : tn('branch.conflict.files', files.length)}>
    {#snippet title()}
      <span class="flex items-center gap-2"><StatusDot tone="danger" beforeText />{heading}</span>
    {/snippet}
  </SyncRow>
</div>

{#if !resolved}
  <ul class="mb-1 max-h-40 overflow-y-auto px-1.5" aria-label={t('branch.conflict.list')}>
    {#each files as file (file.path + file.inProject)}
      {@const slash = file.path.lastIndexOf('/')}
      {@const name = file.path.slice(slash + 1)}
      {@const folder = slash < 0 ? '' : file.path.slice(0, slash)}
      <li>
        <!-- A file inside the project opens in Changes. One outside it (the project is a subfolder of the repository) has no place there. -->
        {#if file.inProject}
          <button type="button" data-menu-item aria-label={t('branch.conflict.open', { file: file.path })} class={cx(FILE, 'text-fg-2 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg focus-visible:bg-hover focus-visible:-outline-offset-2')} onclick={() => open(file.path)}>
            {@render label(name, folder)}
          </button>
        {:else}
          <div title={t('branch.conflict.outside')} class={cx(FILE, 'text-fg-3')}>
            {@render label(name, folder)}
          </div>
        {/if}
      </li>
    {/each}
  </ul>
{/if}

<!-- Two buttons of one width on one line, whatever the harness is called. -->
<div class="grid grid-cols-2 gap-2 px-3.5 pb-3">
  {#if resolved}
    <Button variant="primary" class="min-w-0 justify-center" disabled={busy} loading={branchSync.busy === 'continue'} onclick={() => void branchSync.run('continue')}>
      {kind === 'rebase' ? t('branch.conflict.continue.rebase') : t('branch.conflict.continue.merge')}
    </Button>
  {:else}
    <Button variant="primary" class={cx('min-w-0 justify-center', !operation && !firstInProject && 'col-span-2')} title={t('branch.conflict.askHint')} disabled={busy || session.ended} onclick={() => void ask()}>
      {#snippet leading()}<WandSparkles size={16} aria-hidden="true" />{/snippet}
      {t('branch.conflict.ask', { harness: harnessName })}
    </Button>
  {/if}

  {#if operation}
    <Button class="min-w-0 justify-center" disabled={busy} loading={branchSync.busy === 'abort'} onclick={() => void branchSync.run('abort')}>
      {#snippet leading()}<X size={16} aria-hidden="true" />{/snippet}
      {kind === 'rebase' ? t('branch.conflict.abort.rebase') : t('branch.conflict.abort.merge')}
    </Button>
  {:else if firstInProject}
    <!-- Without a rebase or merge there is nothing to abort: the second button takes the user to the first file to read. -->
    <Button class="min-w-0 justify-center" disabled={busy} onclick={() => open(firstInProject.path)}>
      {#snippet leading()}<FileDiff size={16} aria-hidden="true" />{/snippet}
      {t('branch.conflict.openFirst')}
    </Button>
  {/if}
</div>
