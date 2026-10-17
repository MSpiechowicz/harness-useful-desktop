import type { ScriptDetail } from '../bindings/ScriptDetail';
import type { ScriptRunRequest } from '../bindings/ScriptRunRequest';
import { DEPLOY_CHECK_APPROVED_BODY, summaryOf } from './memory-scripts-fixtures';

function requestId(seed: number): string {
  return seed.toString(16).padStart(32, 'a');
}

/**
 * Two agents asking to run scripts that need a review, as the Memory server's run requests are: one a teammate changed after
 * it was approved here (so the prompt shows what changed) and one this computer has never seen.
 */
export function sampleRunRequests(scripts: ScriptDetail[], now: number = Date.now()): ScriptRunRequest[] {
  const changed = scripts.find((script) => script.name === 'deploy-check');
  const fresh = scripts.find((script) => script.name === 'disk-hogs');
  const requests: ScriptRunRequest[] = [];

  if (changed) {
    requests.push({
      requestId: requestId(1),
      createdAt: new Date(now).toISOString(),
      receivedAt: new Date(now).toISOString(),
      client: 'claude-code',
      script: summaryOf(changed),
      body: changed.body,
      language: changed.language,
      args: ['--env', 'staging'],
      cwd: '/home/mia/work/harness-desktop',
      stdin: { preview: 'staging\n', bytes: 8 },
      previous: { body: DEPLOY_CHECK_APPROVED_BODY, language: changed.language, trustedAt: new Date(now - 9 * 24 * 60 * 60_000).toISOString(), name: null, scope: null, origin: 'approved' },
    });
  }

  if (fresh) {
    requests.push({
      requestId: requestId(2),
      createdAt: new Date(now).toISOString(),
      receivedAt: new Date(now).toISOString(),
      client: 'opencode',
      script: summaryOf(fresh),
      body: fresh.body,
      language: fresh.language,
      args: [],
      cwd: '/home/mia',
      stdin: null,
      previous: null,
    });
  }

  return requests;
}
