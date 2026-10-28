# Subagent details panel — plan and real-CLI checklist

Issue #46. A subagent opens in a side panel with its brief, its tool calls and prose as steps, the tail of its output
and its final report, plus a box to stop it or message it. Data flow and the per-harness table:
[architecture.md](../architecture.md#subagent-details-panel). Privacy rules: [security-privacy.md](../security-privacy.md).
Nothing here was run against a real CLI yet, only fixtures and fakes.

## Decisions

- **Coverage.** Every harness gets as much as its protocol allows. A section or control without data or support is
  hidden, not shown empty.
- **What is tracked.** Subagents started by a skill (a custom subagent type, often parallel, often in the background)
  and by a request are both tracked. Detection never depends on the tool name (`Agent`, `Task`, `task`, `subagent`).
- **Messages.** Direct for OpenCode 1 and 2 (the child session, with the child's own agent and model). Elsewhere an
  orchestrator steer: a prompt with `streamingBehavior` `steer` and the fixed English note `Message for subagent
  "<label>" ("<id>"): <text>`, shown in the transcript as a normal user message. Codex: hidden.
- **Stop.** Per subagent only (Claude `stop_task`, OpenCode 1 abort of the child, OpenCode 2 interrupt of the child).
  The panel never stops the whole chat. Hidden elsewhere.
- **Hooks.** Direct messages skip prompt hooks and memory recall. Orchestrator messages get them.
- **Persistence.** None. Details live in memory and are lost on reload.
- **Title.** `label ?? id`, where the label is the spawn description. Subtitle: agent, model and "background".
- **Shortcut.** Ctrl+] opens the first running subagent, else the newest, and closes the panel when it is open.
- **Redaction.** Texts are redacted before they are cut. The OpenCode server password is masked first.
- **Bounds.** Backend: brief and report 64 Ki characters, prose step 4 Ki, tool name, label and preview 240, output
  tail 400 lines and 64 KiB. Claude keeps 256 tasks per chat (oldest finished first, a new one is not shown while all
  256 run). Webview: 200 steps per subagent, finished subagents forgotten oldest first past 64. A step's output stays only on the latest output step in the panel.
- **Safety.** The chat's own session is never taken for a subagent. A direct OpenCode message is refused (Conflict)
  while the child's agent is unknown.
- **Codex.** Hidden in this build: no recording exists (see [codex.md](codex.md)).

## Real-CLI checklist

Claude Code:

- [ ] Subagent frames arrive for a foreground and a background `Agent` call, with `parent_tool_use_id`.
- [ ] `parent_tool_use_id` equals the `tool_use_id` of `task_started`.
- [ ] A background task's final report: where it appears (notification, last text frame).
- [ ] `stop_task` answers, and the task ends as stopped or killed.
- [ ] Other `task_type` values than `local_agent` and how they look.
- [ ] One `Agent` call can start several tasks (the 1:1 link assumption).
- [ ] The orchestrator steer reaches a running subagent's parent and is passed on.

OpenCode 1:

- [ ] The `task` tool part carries `state.metadata.sessionId` (otherwise the single-running-task fallback decides).
- [ ] `prompt_async` into a running child session is accepted and the child reacts.
- [ ] Abort of a child: the result and the parent's `task` part afterwards.
- [ ] The child's directory is the chat's or its own.
- [ ] Text part ordering against `message.updated` (the agent and model are known before the first text).

OpenCode 2:

- [ ] Child text and tool events arrive on the chat's event stream.
- [ ] A prompt with steer delivery into a running child.
- [ ] Interrupt of a child: the result.
- [ ] Idle can arrive before Busy for a child, and the status does not flicker.

OMP:

- [ ] `recentTools` and `toolCount` semantics, and whether `recentTools` keeps at least 5 entries (steps are lost
  if more tools finish between two progress frames).
- [ ] Whether a per-subagent RPC exists (Stop and direct messages).

Codex:

- [ ] Everything, see [codex.md](codex.md).
