//! Slash-command catalogs: merge precedence, shared commands and per-harness mappings.

use harness_business::harness::{CommandCatalog, Harness, merge_commands, rpc, spec};
use harness_business::harnesses::{Omp, OpenCode, Pi};
use harness_business::model::{AgentActionKind, CommandInvocation, CommandSource, CommandSpec, GuiTarget};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn find<'a>(commands: &'a [CommandSpec], name: &str) -> &'a CommandSpec {
    commands
        .iter()
        .find(|command| command.name == name)
        .unwrap_or_else(|| panic!("/{name} missing"))
}

fn command(name: &str, source: CommandSource) -> CommandSpec {
    spec(name, "", None, source, CommandInvocation::PromptText)
}

/// A catalog whose live list is one command per `{ "name": ... }` object.
struct LiveCatalog;

impl CommandCatalog for LiveCatalog {
    fn native(&self) -> Vec<CommandSpec> {
        vec![command("model", CommandSource::Builtin)]
    }

    fn parse_live(&self, raw: &[Value]) -> Vec<CommandSpec> {
        raw.iter()
            .filter_map(|item| item["name"].as_str())
            .map(|name| command(name, CommandSource::Skill))
            .collect()
    }
}

#[test]
fn native_beats_live_and_live_beats_shared() {
    let merged = LiveCatalog.merged(&[
        json!({ "name": "model" }),
        json!({ "name": "new" }),
        json!({ "name": "us-plan" }),
    ]);

    let names: Vec<(&str, CommandSource)> = merged
        .iter()
        .map(|command| (command.name.as_str(), command.source))
        .collect();
    assert_eq!(
        names,
        [
            ("model", CommandSource::Builtin),
            ("new", CommandSource::Skill),
            ("us-plan", CommandSource::Skill),
            ("compact", CommandSource::Shared),
            ("thinking", CommandSource::Shared),
            ("name", CommandSource::Shared),
            ("abort", CommandSource::Shared),
        ]
    );
}

#[test]
fn merging_keeps_the_first_command_of_each_name() {
    let merged = merge_commands(
        vec![command("a", CommandSource::Builtin)],
        vec![command("a", CommandSource::Skill), command("b", CommandSource::Skill)],
        vec![command("b", CommandSource::Shared), command("c", CommandSource::Shared)],
    );

    let sources: Vec<CommandSource> = merged.iter().map(|command| command.source).collect();
    assert_eq!(
        sources,
        [CommandSource::Builtin, CommandSource::Skill, CommandSource::Shared]
    );
}

#[test]
fn shared_commands_are_always_offered() {
    for catalog in [Omp.catalog(), Pi.catalog()] {
        let merged = catalog.merged(&[]);

        for name in ["new", "compact", "model", "thinking", "name", "abort"] {
            find(&merged, name);
        }
        assert_eq!(
            find(&merged, "thinking").invocation,
            CommandInvocation::Gui {
                target: GuiTarget::ThinkingPicker
            }
        );
        assert_eq!(find(&merged, "abort").invocation, rpc(AgentActionKind::Abort));
    }
}

#[test]
fn omp_commands_run_as_prompt_text_except_the_model_picker() {
    let merged = Omp.catalog().merged(&[]);

    assert_eq!(
        find(&merged, "model").invocation,
        CommandInvocation::Gui {
            target: GuiTarget::ModelPicker
        }
    );
    assert_eq!(find(&merged, "compact").invocation, CommandInvocation::PromptText);
    assert_eq!(find(&merged, "compact").source, CommandSource::Builtin);
    assert_eq!(find(&merged, "review").source, CommandSource::Custom);

    // `/new` is TUI-only in OMP, so the shared RPC mapping is used.
    assert_eq!(find(&merged, "new").invocation, rpc(AgentActionKind::NewSession));
    assert_eq!(find(&merged, "new").source, CommandSource::Shared);
}

#[test]
fn pi_terminal_builtins_map_to_rpc_views_or_unsupported() {
    let merged = Pi.catalog().merged(&[]);
    let invocation = |name: &str| find(&merged, name).invocation.clone();

    assert_eq!(invocation("new"), rpc(AgentActionKind::NewSession));
    assert_eq!(invocation("name"), rpc(AgentActionKind::SetSessionName));
    assert_eq!(invocation("copy"), rpc(AgentActionKind::GetLastAssistantText));
    assert_eq!(
        invocation("tree"),
        CommandInvocation::Gui {
            target: GuiTarget::SessionTree
        }
    );

    for name in ["logout", "trust", "reload", "share", "quit"] {
        assert!(
            matches!(invocation(name), CommandInvocation::Unsupported { .. }),
            "/{name} must be unsupported"
        );
    }

    let prompt_text = merged
        .iter()
        .filter(|command| command.invocation == CommandInvocation::PromptText)
        .count();
    assert_eq!(prompt_text, 0, "PI builtins are never sent as prompt text");
}

#[test]
fn opencode_terminal_commands_map_to_chat_actions_views_or_unsupported() {
    let merged = OpenCode.catalog().merged(&[]);
    let invocation = |name: &str| find(&merged, name).invocation.clone();
    let view = |target| CommandInvocation::Gui { target };

    assert_eq!(invocation("new"), rpc(AgentActionKind::NewSession));
    assert_eq!(invocation("compact"), rpc(AgentActionKind::Compact));
    assert_eq!(invocation("rename"), rpc(AgentActionKind::SetSessionName));
    assert_eq!(invocation("name"), rpc(AgentActionKind::SetSessionName));
    assert_eq!(invocation("export"), rpc(AgentActionKind::ExportHtml));
    assert_eq!(invocation("copy"), rpc(AgentActionKind::GetLastAssistantText));
    assert_eq!(invocation("abort"), rpc(AgentActionKind::Abort));
    assert_eq!(invocation("fork"), view(GuiTarget::ForkPicker));
    assert_eq!(invocation("sessions"), view(GuiTarget::ResumePicker));
    assert_eq!(invocation("resume"), view(GuiTarget::ResumePicker));
    assert_eq!(invocation("models"), view(GuiTarget::ModelPicker));
    assert_eq!(invocation("model"), view(GuiTarget::ModelPicker));
    assert_eq!(invocation("variants"), view(GuiTarget::ThinkingPicker));
    assert_eq!(invocation("thinking"), view(GuiTarget::ThinkingPicker));
    assert_eq!(invocation("agents"), view(GuiTarget::AgentPicker));
    assert_eq!(invocation("agent"), view(GuiTarget::AgentPicker));

    for name in [
        "undo", "redo", "timeline", "move", "mcps", "skills", "connect", "themes", "editor", "status", "help", "exit",
    ] {
        assert!(
            matches!(invocation(name), CommandInvocation::Unsupported { reason } if !reason.contains("coming soon")),
            "/{name} must be unsupported with a reason"
        );
    }

    let prompt_text = merged
        .iter()
        .filter(|command| command.invocation == CommandInvocation::PromptText)
        .count();
    assert_eq!(prompt_text, 0, "terminal commands are never sent as prompt text");
}

#[test]
fn opencode_server_commands_come_from_the_live_list_as_prompt_text() {
    let live = [
        json!({ "name": "init", "description": "guided AGENTS.md setup", "source": "command" }),
        json!({ "name": "review", "source": "command", "subtask": true }),
        json!({ "name": "github:triage", "source": "mcp" }),
        json!({ "name": "customize-opencode", "source": "skill" }),
        json!({ "name": "export", "source": "command" }),
        json!({ "name": "tui-only", "source": "tui" }),
    ];

    assert!(
        OpenCode
            .catalog()
            .merged(&[])
            .iter()
            .all(|command| command.name != "init")
    );

    let merged = OpenCode.catalog().merged(&live);
    for (name, source) in [
        ("init", CommandSource::Custom),
        ("review", CommandSource::Custom),
        ("github:triage", CommandSource::McpPrompt),
        ("customize-opencode", CommandSource::Skill),
    ] {
        assert_eq!(find(&merged, name).source, source, "/{name}");
        assert_eq!(find(&merged, name).invocation, CommandInvocation::PromptText, "/{name}");
    }
    assert_eq!(find(&merged, "init").description, "guided AGENTS.md setup");

    assert_eq!(
        find(&merged, "export").invocation,
        rpc(AgentActionKind::ExportHtml),
        "a terminal command keeps its mapping"
    );
    assert!(merged.iter().all(|command| command.name != "tui-only"));
}
