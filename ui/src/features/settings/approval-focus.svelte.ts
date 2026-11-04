import { nav } from '$lib/stores/nav.svelte';

/** A request to show the team approval row in Settings. The row takes it once it is on screen (the team database may still be loading) and scrolls to itself. */
class ApprovalFocus {
  requested = $state(false);

  /** The row has taken the request. */
  take(): void {
    this.requested = false;
  }
}

export const approvalFocus = new ApprovalFocus();

/** Opens Settings at the team approval row of Remote knowledge, scrolls to it and flashes it. For the "Change in Settings" link of the import dialog. */
export function showTeamApproval(): void {
  approvalFocus.requested = true;
  nav.go('settings');
}
