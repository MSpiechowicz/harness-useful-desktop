import type { AppInfo } from '$lib/ipc/bindings/AppInfo';
import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';

/**
 * Plain text for bug reports: app version, OS and architecture, and each harness's install status and version.
 * No paths, project names or preferences are included.
 */
export function diagnosticsText(info: AppInfo | null, harnesses: HarnessSummary[]): string {
  const lines = [info ? `${info.name} ${info.version} (${info.os} ${info.arch}${info.debug ? ', debug build' : ''})` : 'Harness Desktop (version unknown)'];

  for (const { info: harness, detection } of harnesses) {
    lines.push(`${harness.name}: ${harnessStatus(detection)}`);
  }

  return lines.join('\n');
}

function harnessStatus(detection: HarnessSummary['detection']): string {
  if (!detection.installed) {
    return 'not installed';
  }

  const version = detection.version ?? 'version unknown';

  return `installed, ${version} (${detection.versionSupport})`;
}
