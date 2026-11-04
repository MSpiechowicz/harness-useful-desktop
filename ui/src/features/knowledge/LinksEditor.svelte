<script lang="ts">
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import Lock from '@lucide/svelte/icons/lock';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import Unlink from '@lucide/svelte/icons/unlink';
  import X from '@lucide/svelte/icons/x';
  import { flip } from 'svelte/animate';

  import Badge from '$lib/components/Badge.svelte';
  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import type { ScopeOption } from '$lib/components/scope/scope-options';
  import { rise, scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeLink } from '$lib/ipc/bindings/KnowledgeLink';
  import { uid } from '$lib/utils/ids';
  import { isFolderKey } from '$lib/utils/knowledge-local-path';

  import AddPath from './AddPath.svelte';
  import { pathsBadge, repoLabel } from './knowledge-format';
  import { repoChoices } from './knowledge-repos';
  import LinkRepoPicker from './LinkRepoPicker.svelte';

  interface Props {
    /** The repositories and paths on screen. */
    links: KnowledgeLink[];
    /** The links after a change. Where it saves, the caller says so once it did. */
    onchange: (links: KnowledgeLink[]) => void;
    /** The entries of the repository menu (see `knowledgeScopeOptions`). */
    options: ScopeOption[];
    /** The document goes to (or is kept in) the team database, which only a `git:` project can be linked to: a folder without a git remote is listed but off, with the reason. */
    team?: boolean;
    /** The user may not change the document (a read-only role on the team database): every control is off, with the reason under them. */
    readOnly?: boolean;
  }

  let { links, onchange, options, team = false, readOnly = false }: Props = $props();

  const readOnlyId = uid('knowledge-links-read-only');
  const choices = $derived(repoChoices(options, links.map((link) => link.repo), team));

  /** What a repository is called in the menu (a project's folder name), else as its key reads: a folder by its last name, never its path. */
  function nameOf(key: string): string {
    return options.find((option) => option.value === `repo:${key}`)?.label ?? repoLabel(key);
  }

  function link(key: string): void {
    onchange([...links, { repo: key, globs: [] }]);
  }

  function unlink(repo: string): void {
    onchange(links.filter((entry) => entry.repo !== repo));
  }

  function addPath(repo: string, glob: string): void {
    onchange(links.map((entry) => (entry.repo === repo && !entry.globs.includes(glob) ? { ...entry, globs: [...entry.globs, glob] } : entry)));
  }

  function removePath(repo: string, glob: string): void {
    onchange(links.map((entry) => (entry.repo === repo ? { ...entry, globs: entry.globs.filter((existing) => existing !== glob) } : entry)));
  }
</script>

<div class="flex flex-col gap-3">

  {#each links as entry (entry.repo)}
    {@const name = nameOf(entry.repo)}
    <div in:rise={{ y: 6, duration: 220 }} animate:flip={{ duration: scaled(220) }} class="flex flex-col gap-2.5 rounded-lg border border-line bg-surface-2 p-3">
      <div class="flex items-center gap-2.5">
        <GitBranch size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />
        <span class="min-w-0 flex-1 truncate text-sm font-medium text-fg">{name}</span>
        <Badge size="lg" tone="neutral">{pathsBadge(entry.globs.length)}</Badge>
      </div>

      {#if team && isFolderKey(entry.repo)}
        <p class="flex items-start gap-2 text-xs text-warning" role="status"><TriangleAlert size={ICON.sm} class="mt-px shrink-0" aria-hidden="true" />{t('knowledge.links.folderNoRemote')}</p>
      {/if}

      {#if entry.globs.length > 0}
        <ul class="flex flex-wrap gap-1.5">
          {#each entry.globs as glob (glob)}
            <li in:rise={{ y: 4, duration: 160 }} class="flex h-7 items-center gap-1.5 rounded-full bg-surface-3 pr-2 pl-3 font-mono text-[13px] text-fg-2">
              <span class="min-w-0 truncate">{glob}</span>
              <button
                type="button"
                aria-label={t('knowledge.links.removePath', { path: glob })}
                disabled={readOnly}
                aria-describedby={readOnly ? readOnlyId : undefined}
                class="grid size-5 shrink-0 place-items-center rounded-full text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-press hover:text-fg disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:bg-transparent"
                onclick={() => removePath(entry.repo, glob)}
              >
                <X size={14} aria-hidden="true" />
              </button>
            </li>
          {/each}
        </ul>
      {/if}

      <div class="flex items-center gap-2">
        <AddPath repo={name} disabled={readOnly} describedby={readOnly ? readOnlyId : undefined} onadd={(glob) => addPath(entry.repo, glob)} />
        <Button
          variant="dangerQuiet"
          class="shrink-0"
          aria-label={t('knowledge.links.unlink', { repo: name })}
          disabled={readOnly}
          aria-describedby={readOnly ? readOnlyId : undefined}
          onclick={() => unlink(entry.repo)}
        >
          {#snippet leading()}<Unlink size={ICON.sm} aria-hidden="true" />{/snippet}
          {t('knowledge.links.unlinkShort')}
        </Button>
      </div>
    </div>
  {/each}

  <div class="flex">
    <LinkRepoPicker {choices} onpick={link} disabled={readOnly} describedby={readOnly ? readOnlyId : undefined} />
  </div>

  {#if readOnly}
    <p id={readOnlyId} in:rise={{ y: 4, duration: 200 }} class="flex items-center gap-2 text-sm text-fg-3"><Lock size={ICON.sm} class="shrink-0" aria-hidden="true" />{t('knowledge.links.readOnly')}</p>
  {/if}
</div>
