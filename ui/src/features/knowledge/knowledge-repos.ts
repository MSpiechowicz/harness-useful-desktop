import { buildScopeOptions, choiceKey, type ScopeOption } from '$lib/components/scope/scope-options';
import { t } from '$lib/i18n/i18n.svelte';
import type { KnowledgeReposPage } from '$lib/ipc/bindings/KnowledgeReposPage';
import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';

import { repoLabel } from './knowledge-format';

/**
 * The entries of the repository menus: All and Global (counted when the server answered), the projects on this computer and the repositories only the
 * documents know. The Scope filter lists all of them. `repos` below picks the repositories out for a menu that links one.
 */
export function knowledgeScopeOptions(projects: ProjectScope[], page: KnowledgeReposPage | null, allCount: number | undefined): ScopeOption[] {
  const answered = page !== null && page.availability.type === 'available' ? page : null;

  return buildScopeOptions({
    projects,
    remote: answered ? answered.repos.map((repo) => ({ key: repo.key, label: repoLabel(repo.key), count: repo.docs })) : null,
    globalCount: answered?.global,
    allCount,
    labels: { all: t('components.scope.all'), global: t('components.scope.global'), remoteOnly: t('components.scope.remoteOnly') },
  });
}

/** A repository a document can link to: the one an option stands for, by its key. */
export interface RepoChoice {
  key: string;
  label: string;
  hint?: string;
}

/** The repositories among the options that no link has yet. A project without a repository key cannot be linked: a link names a repository by its key. */
export function repoChoices(options: ScopeOption[], linked: readonly string[]): RepoChoice[] {
  return options.flatMap((option) => {
    const key = choiceKey(option.value);

    return key === null || linked.includes(key) ? [] : [{ key, label: option.label, hint: option.hint }];
  });
}
