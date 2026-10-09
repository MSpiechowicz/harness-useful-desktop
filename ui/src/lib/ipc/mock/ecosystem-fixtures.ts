import type { CliResult } from '../bindings/CliResult';
import type { DashboardResult } from '../bindings/DashboardResult';
import type { EcosystemStatus } from '../bindings/EcosystemStatus';
import branchCost from './fixtures/ecosystem/branch_cost.json';
import breakdown from './fixtures/ecosystem/breakdown.json';
import breakdownProvider from './fixtures/ecosystem/breakdown.provider.json';
import budgetStatus from './fixtures/ecosystem/budget_status.json';
import cliDryRun from './fixtures/ecosystem/cli_result_dry_run.json';
import cliFailed from './fixtures/ecosystem/cli_result_failed.json';
import cliOk from './fixtures/ecosystem/cli_result_ok.json';
import failures from './fixtures/ecosystem/failures.json';
import limits from './fixtures/ecosystem/limits.json';
import sessionCost from './fixtures/ecosystem/session_cost.json';
import sessionCostError from './fixtures/ecosystem/session_cost.error.json';
import status from './fixtures/ecosystem/status.json';
import tips from './fixtures/ecosystem/tips.json';
import usageSummary from './fixtures/ecosystem/usage_summary.json';
import usageSummary30d from './fixtures/ecosystem/usage_summary.30d.json';
import usageSummary7d from './fixtures/ecosystem/usage_summary.7d.json';

/**
 * Backend-generated samples of the ecosystem commands (see `business`), served as they are or
 * adapted by the handlers. The JSON module types are widened, hence the casts.
 */
export const ecosystemFixtures = {
  status: status as unknown as EcosystemStatus,
  usage: { today: usageSummary, '7d': usageSummary7d, '30d': usageSummary30d } as unknown as Record<string, DashboardResult>,
  breakdown: { model: breakdown, provider: breakdownProvider } as unknown as Record<string, DashboardResult>,
  sessionCost: sessionCost as unknown as DashboardResult,
  sessionCostError: sessionCostError as unknown as DashboardResult,
  branchCost: branchCost as unknown as DashboardResult,
  limits: limits as unknown as DashboardResult,
  budgetStatus: budgetStatus as unknown as DashboardResult,
  failures: failures as unknown as DashboardResult,
  tips: tips as unknown as DashboardResult,
  cli: { ok: cliOk, dryRun: cliDryRun, failed: cliFailed } as unknown as Record<'ok' | 'dryRun' | 'failed', CliResult>,
};
