import type { DiffScope } from '../bindings/DiffScope';
import type { FileDiff } from '../bindings/FileDiff';
import type { GitStatus } from '../bindings/GitStatus';
import type { ReviewRequest } from '../bindings/ReviewRequest';
import { IpcFailure } from '../transport';
import { fixtures } from './fixtures';
import type { MockState } from './state';
import type { Handlers } from './types';

type GitCommands = 'git_status' | 'git_branches' | 'git_diff' | 'review_compose';

const MAX_COMMENTS = 100;
const NOT_A_REPO: GitStatus = { isRepo: false, branch: null, head: null, upstream: null, ahead: 0, behind: 0, entries: [] };

const SCOPES: Record<DiffScope, (diff: FileDiff) => boolean> = {
  all: () => true,
  staged: (diff) => diff.staged,
  unstaged: (diff) => !diff.staged && diff.status !== 'untracked',
  untracked: (diff) => diff.status === 'untracked',
};

const FENCE_LANGUAGES: Record<string, string> = { rs: 'rust', ts: 'ts', js: 'js', md: 'markdown', json: 'json', svelte: 'svelte', toml: 'toml', py: 'python' };

/** True when the project is a git repository, as far as the folder list says. */
function isRepo(state: MockState, project: string): boolean {
  const known = state.active?.path === project ? state.active : state.recents.find((recent) => recent.path === project);

  return known?.isGitRepo ?? false;
}

/** Deterministic Markdown in the shape the real composer produces. */
function composePrompt(request: ReviewRequest): string {
  const parts = ['Please address these review comments on the current changes.'];
  if (request.note) {
    parts.push(request.note);
  }

  const files = [...new Set(request.comments.map((comment) => comment.path))];
  for (const path of files) {
    parts.push(`### ${path}`);

    const extension = path.split('.').at(-1) ?? '';
    for (const comment of request.comments.filter((candidate) => candidate.path === path)) {
      const range = comment.startLine === comment.endLine ? `Line ${comment.startLine}` : `Lines ${comment.startLine}–${comment.endLine}`;
      parts.push(`- ${range} (${comment.side}): ${comment.body}`);
      parts.push(`\`\`\`${FENCE_LANGUAGES[extension] ?? ''}\n${comment.snippet.join('\n')}\n\`\`\``);
    }
  }

  return `${parts.join('\n\n')}\n`;
}

/** In-memory git: the generated status, branches and diffs, filtered the way the real commands filter. */
export function gitHandlers(state: MockState): Pick<Handlers, GitCommands> {
  return {
    git_status: ({ project }) => {
      state.requireKnown(project);

      return structuredClone(isRepo(state, project) ? fixtures.git.status : NOT_A_REPO);
    },

    git_branches: ({ project }) => {
      state.requireKnown(project);

      return structuredClone(isRepo(state, project) ? fixtures.git.branches : []);
    },

    git_diff: ({ project, request }) => {
      state.requireKnown(project);
      if (!isRepo(state, project)) {
        return [];
      }

      const wanted = new Set(request.paths);
      const diffs = fixtures.git.diff.filter((diff) => SCOPES[request.scope](diff) && (wanted.size === 0 || wanted.has(diff.path)));

      return structuredClone(diffs);
    },

    review_compose: ({ request }) => {
      if (request.comments.length === 0 || request.comments.length > MAX_COMMENTS) {
        throw IpcFailure.from({ code: 'invalidInput', message: `A review needs between 1 and ${MAX_COMMENTS} comments.` });
      }

      const prompt = composePrompt(request);
      const files = new Set(request.comments.map((comment) => comment.path));

      return { prompt, bytes: new TextEncoder().encode(prompt).length, fileCount: files.size, commentCount: request.comments.length };
    },
  };
}
