//! Live slash-command lists reported over RPC, parsed and merged into each harness catalog.

use harness_business::harness::Harness;
use harness_business::harnesses::{Omp, Pi};
use harness_business::model::{CommandInvocation, CommandSource, CommandSpec, GuiTarget};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn find<'a>(commands: &'a [CommandSpec], name: &str) -> Option<&'a CommandSpec> {
    commands.iter().find(|command| command.name == name)
}

fn omp_live() -> Vec<Value> {
    vec![
        json!({ "name": "skill:tdd", "description": "Test first", "source": "skill" }),
        json!({ "name": "/deploy", "description": "Ship it", "input": { "hint": "<env>" }, "source": "extension" }),
        json!({ "name": "triage", "source": "custom", "aliases": ["t"], "subcommands": [{ "name": "all" }] }),
        json!({ "name": "gh:summarize", "source": "mcp_prompt" }),
        json!({ "name": "notes", "source": "file" }),
        json!({ "name": "model", "description": "live model", "source": "builtin" }),
        json!({ "name": "has space", "source": "skill" }),
        json!({ "name": "", "source": "skill" }),
        json!({ "name": "mystery", "source": "telepathy" }),
        json!({ "name": "skill:tdd", "description": "duplicate", "source": "skill" }),
    ]
}

#[test]
fn omp_live_commands_map_their_source_and_run_as_prompt_text() {
    let live = Omp.catalog().parse_live(&omp_live());
    let names: Vec<&str> = live.iter().map(|command| command.name.as_str()).collect();

    assert_eq!(
        names,
        ["skill:tdd", "deploy", "triage", "gh:summarize", "notes", "model"]
    );
    assert_eq!(find(&live, "deploy").unwrap().argument_hint.as_deref(), Some("<env>"));
    assert_eq!(find(&live, "triage").unwrap().source, CommandSource::Custom);
    assert_eq!(find(&live, "gh:summarize").unwrap().source, CommandSource::McpPrompt);
    assert_eq!(find(&live, "notes").unwrap().source, CommandSource::File);
    assert_eq!(
        find(&live, "skill:tdd").unwrap().invocation,
        CommandInvocation::PromptText
    );
    assert_eq!(
        find(&live, "model").unwrap().invocation,
        CommandInvocation::Gui {
            target: GuiTarget::ModelPicker
        }
    );
}

#[test]
fn omp_merge_keeps_native_builtins_and_adds_live_commands() {
    let merged = Omp.catalog().merged(&omp_live());

    assert_eq!(
        find(&merged, "model").unwrap().description,
        "Show current model selection"
    );
    assert_eq!(find(&merged, "skill:tdd").unwrap().source, CommandSource::Skill);
    assert_eq!(find(&merged, "review").unwrap().source, CommandSource::Custom);
    assert_eq!(find(&merged, "new").unwrap().source, CommandSource::Shared);
    assert!(find(&merged, "mystery").is_none());
}

#[test]
fn pi_live_commands_are_prompts_skills_and_extensions_and_drop_source_paths() {
    let raw = [
        json!({ "name": "fix", "description": "Fix tests", "source": "prompt", "sourceInfo": { "path": "/home/demo/.pi/prompts/fix.md" } }),
        json!({ "name": "skill:tdd", "source": "skill", "sourceInfo": { "path": "/home/demo/.pi/agent/skills/tdd" } }),
        json!({ "name": "hello", "source": "extension" }),
        json!({ "name": "compact", "description": "extension override", "source": "extension" }),
        json!({ "name": "settings", "source": "builtin" }),
    ];

    let live = Pi.catalog().parse_live(&raw);
    let sources: Vec<(&str, CommandSource)> = live
        .iter()
        .map(|command| (command.name.as_str(), command.source))
        .collect();
    assert_eq!(
        sources,
        [
            ("fix", CommandSource::Prompt),
            ("skill:tdd", CommandSource::Skill),
            ("hello", CommandSource::Extension),
            ("compact", CommandSource::Extension),
        ]
    );
    assert!(
        live.iter()
            .all(|command| command.invocation == CommandInvocation::PromptText)
    );
    assert!(!serde_json::to_string(&live).unwrap().contains("/home/demo"));

    // The PI builtin keeps its RPC mapping over a live command of the same name.
    let merged = Pi.catalog().merged(&raw);
    assert_eq!(find(&merged, "compact").unwrap().source, CommandSource::Builtin);
    assert_eq!(find(&merged, "hello").unwrap().source, CommandSource::Extension);
}
