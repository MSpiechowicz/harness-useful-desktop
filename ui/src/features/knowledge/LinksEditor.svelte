<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import GitBranch from '@lucide/svelte/icons/git-branch';
  import X from '@lucide/svelte/icons/x';
  import { flip } from 'svelte/animate';

  import { ICON } from '$lib/components/icon';
  import IconButton from '$lib/components/IconButton.svelte';
  import type { ScopeOption } from '$lib/components/scope/scope-options';
  import { rise, scaled } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeLink } from '$lib/ipc/bindings/KnowledgeLink';

  import AddPath from './AddPath.svelte';
  import { repoLabel } from './knowledge-format';
  import { repoChoices } from './knowledge-repos';
  import LinkRepoPicker from './LinkRepoPicker.svelte';

  interface Props {
    /** The repositories and paths on screen. */
    links: KnowledgeLink[];
    /** The links after a change. Where it saves, `saved` says so once it did. */
    onchange: (links: KnowledgeLink[]) => void;
    /** The entries of the repository menu (see `knowledgeScopeOptions`). */
    options: ScopeOption[];
    /** Shows a quiet "Saved" after the buttons: the editor of a document saves with every change. */
    saved?: boolean;
    /** Said while nothing is linked. */
    globalNote?: string;
  }

  let { links, onchange, options, saved = false, globalNote }: Props = $props();

  const choices = $derived(repoChoices(options, links.map((link) => link.repo)));

  /** What a repository is called in the menu (a project's folder name), else as its key reads. */
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
  {#if links.length === 0 && globalNote}
    <p class="text-sm text-fg-3">{globalNote}</p>
  {/if}

  {#each links as entry (entry.repo)}
    {@const name = nameOf(entry.repo)}
    <div in:rise={{ y: 6, duration: 220 }} animate:flip={{ duration: scaled(220) }} class="flex flex-col gap-2.5 rounded-lg border border-line bg-surface-2 p-3">
      <div class="flex items-center gap-2.5">
        <GitBranch size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />
        <span class="min-w-0 flex-1 truncate text-sm font-medium text-fg" title={entry.repo}>{name}</span>
        <IconButton icon={X} iconSize={ICON.sm} label={t('knowledge.links.unlink', { repo: name })} class="-my-1 -mr-1.5 shrink-0" onclick={() => unlink(entry.repo)} />
      </div>

      {#if entry.globs.length === 0}
        <p class="text-xs text-fg-3">{t('knowledge.links.wholeRepo')}</p>
      {:else}
        <ul class="flex flex-wrap gap-1.5">
          {#each entry.globs as glob (glob)}
            <li in:rise={{ y: 4, duration: 160 }} class="flex h-6 items-center gap-1 rounded-sm bg-surface-3 pr-0.5 pl-2 font-mono text-xs text-fg-2">
              <span class="min-w-0 truncate">{glob}</span>
              <button
                type="button"
                aria-label={t('knowledge.links.removePath', { path: glob })}
                class="grid size-5 shrink-0 place-items-center rounded-sm text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-press hover:text-fg"
                onclick={() => removePath(entry.repo, glob)}
              >
                <X size={12} aria-hidden="true" />
              </button>
            </li>
          {/each}
        </ul>
      {/if}

      <div class="-mb-1 flex"><AddPath repo={name} onadd={(glob) => addPath(entry.repo, glob)} /></div>
    </div>
  {/each}

  <div class="flex items-center gap-3">
    <LinkRepoPicker {choices} onpick={link} />

    {#if saved}
      <span in:rise={{ y: 4, duration: 200 }} class="flex items-center gap-1.5 text-sm text-success" role="status">
        <Check size={ICON.sm} aria-hidden="true" />
        {t('knowledge.links.saved')}
      </span>
    {/if}
  </div>
</div>
