import { buildScopeOptions, choiceKey, type ScopeOption } from '$lib/components/scope/scope-options';
import { t } from '$lib/i18n/i18n.svelte';
import type { KnowledgeDocument } from '$lib/ipc/bindings/KnowledgeDocument';
import type { KnowledgeReposPage } from '$lib/ipc/bindings/KnowledgeReposPage';
import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';
import type { DocumentRouting } from '$lib/utils/knowledge-routing';
import { isFolderKey } from '$lib/utils/knowledge-local-path';

import { repoLabel } from './knowledge-format';
import { keptRemote } from './knowledge-replace';

/**
 * The entries of the repository menus: All and Global (counted when the server answered), the projects on this computer and the repositories only the
 * documents know. The Scope filter lists all of them. `repos` below picks the repositories out for a menu that links one.
 */
export function knowledgeScopeOptions(projects: ProjectScope[], page: KnowledgeReposPage | null, allCount: number | undefined): ScopeOption[] {
  const answered = page !== null && page.availability.type === 'available' ? page : null;

  const options = buildScopeOptions({
    projects,
    remote: answered ? answered.repos.map((repo) => ({ key: repo.key, label: repoLabel(repo.key), count: repo.docs })) : null,
    globalCount: answered?.global,
    allCount,
    labels: { all: t('components.scope.all'), global: t('components.scope.global'), remoteOnly: t('components.scope.remoteOnly') },
  });

  return options.map(withoutFullPath);
}

/** Two projects of one name are told apart by their folder. That hint shows the parent folder and the folder, never the whole path to it. */
function withoutFullPath(option: ScopeOption): ScopeOption {
  if (option.path === undefined || option.hint !== option.path) {
    return option;
  }

  return { ...option, hint: option.path.split(/[\\/]/).filter(Boolean).slice(-2).join('/') };
}

/**
 * Whether a document kept in the team database: the document says where it is kept while both places are in use, else the routing says. Nothing is
 * in the team database while there is none (it is off, or not read yet).
 */
export function inTeamDatabase(doc: Pick<KnowledgeDocument, 'place'>, remote: (DocumentRouting & { enabled: boolean }) | null): boolean {
  return remote !== null && remote.enabled && keptRemote(doc, remote);
}

/** A repository a document can link to: the one an option stands for, by its key. */
export interface RepoChoice {
  key: string;
  label: string;
  hint?: string;
  /** Why it cannot be chosen: shown, but off. */
  reason?: string;
}

/**
 * The repositories among the options that no link has yet. A project without a repository key cannot be linked: a link names a repository by its key.
 * For documents that go to the team database (`team`) a folder without a git remote (a `path:` key) is listed but off, with the reason: teammates could
 * not find it, and the server refuses it.
 */
export function repoChoices(options: ScopeOption[], linked: readonly string[], team = false): RepoChoice[] {
  return options.flatMap((option) => {
    const key = choiceKey(option.value);

    if (key === null || linked.includes(key)) {
      return [];
    }

    const reason = team && isFolderKey(key) ? t('knowledge.links.folderNoRemote') : undefined;

    return [{ key, label: option.label, hint: option.hint, ...(reason ? { reason } : {}) }];
  });
}
