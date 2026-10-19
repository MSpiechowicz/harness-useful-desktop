import type { AgentEnvelope } from '../bindings/AgentEnvelope';
import type { AgentSessionInfo } from '../bindings/AgentSessionInfo';
import type { EventSink } from '../transport';
import { previewSchedule } from './routine-schedule';
import type { MockRoutines } from './routine-state';
import type { Handlers } from './types';

type RoutineCommands =
  | 'routines_list'
  | 'routine_get'
  | 'routine_save'
  | 'routine_delete'
  | 'routines_delete_all'
  | 'routine_approve'
  | 'routine_reject'
  | 'routines_listen'
  | 'routine_run_now'
  | 'routine_run_stop'
  | 'routine_runs'
  | 'routine_schedule_preview'
  | 'routines_team_refresh'
  | 'routines_share'
  | 'routines_team_remove'
  | 'routines_team_remove_mine'
  | 'routines_team_copy'
  | 'routines_team_enable'
  | 'routine_run_attach'
  | 'routines_workspace';

/** Opens a mock agent session for a run's chat: it says the prompt, and when the run asks, shows its question. */
export type AttachRunChat = (info: AgentSessionInfo, onEvent: EventSink<AgentEnvelope>, chat: { prompt: string; asks: boolean }) => AgentSessionInfo;

/** The routines folder, the runs and the schedule preview of the mock backend: the commands of the Routines view, without a folder. */
export function routineHandlers(routines: MockRoutines, attach: AttachRunChat): Pick<Handlers, RoutineCommands> {
  return {
    routines_list: ({ project }) => routines.list(project),
    routine_get: ({ id }) => routines.get(id),
    routine_save: ({ draft }) => routines.save(draft),
    routine_delete: ({ id, sha256 }) => routines.delete(id, sha256),
    routines_delete_all: ({ project }) => routines.deleteAll(project ?? null),
    routine_approve: ({ id, sha256 }) => routines.approve(id, sha256),
    routine_reject: ({ id, sha256 }) => routines.reject(id, sha256),
    routines_listen: ({ onEvent }) => routines.listen(onEvent),
    routine_run_now: ({ id }) => routines.runNow(id),
    routine_run_stop: ({ runId }) => routines.stop(runId),
    routine_runs: ({ query }) => routines.runsPage(query),
    routine_schedule_preview: ({ when }) => previewSchedule(when),
    routines_team_refresh: ({ project }) => routines.teamRefresh(project ?? null),
    routines_share: ({ id, sha256 }) => routines.share(id, sha256),
    routines_team_remove: ({ id, sha256 }) => routines.teamRemove(id, sha256),
    routines_team_remove_mine: () => routines.teamRemoveMine(),
    routines_team_copy: ({ id, sha256, project }) => routines.teamCopy(id, sha256, project ?? null),
    routines_team_enable: ({ id, enabled, project }) => routines.teamEnable(id, enabled, project ?? null),
    routine_run_attach: ({ runId, onEvent }) => attach(routines.attach(runId), onEvent, routines.conversation(runId)),
    routines_workspace: () => routines.workspace(),
  };
}
