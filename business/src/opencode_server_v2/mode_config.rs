//! An approval mode as configuration of the chat's own OpenCode 2 `opencode serve`
//! (`OPENCODE_CONFIG_CONTENT`). Pure: no I/O.
//!
//! OpenCode 2 reads `OPENCODE_CONFIG_CONTENT` after the user's global and project configuration;
//! only managed settings come later (`config.ts` `load`). Its agent configuration
//! (`config/plugin/agent.ts`) appends every document's top-level `permissions` to every agent,
//! then each document's `agents.<id>.permissions` to that agent, in document order, so the rules
//! the chat writes come after the user's for every agent. Rules are lists: no key order matters, as
//! it does on OpenCode 1. The chat starts its server once without the mode, reads every agent's
//! rules (`GET /api/agent`), and starts it again with each agent's mode rules ([`mode_rules`]) as
//! `agents.<id>.permissions` ([`mode_config`]). An agent the configuration names is created when
//! it does not exist, so only the agents the server listed are named. Nothing is written to
//! OpenCode's files or database.
//!
//! [`merge_content`] keeps an inherited `OPENCODE_CONFIG_CONTENT`: its own rules for an agent stay
//! before the mode's. One that configures an agent the OpenCode 1 way (`agent.<id>`, `mode.<id>`)
//! is refused, as OpenCode 2 drops it for the native entry of the same document.
//!
//! [`check_applied`] checks every agent the server reports with the mode: the mode's rules are
//! there in one piece, only denies follow them (OpenCode 2.1.2's browser connection appends a
//! `browser` deny to every agent once it loads, a few hundred milliseconds after the agents), and
//! the rules before them are the probe's. The mode's rules start with a catch-all, so the rules
//! before them decide nothing, but the mode keeps the agent's denies only when it saw them: denies
//! at the end of either list may differ only when they still apply (in the mode's rules or after
//! them). Rules of one effect next to each other may come in another order, which decides nothing.
//! Anything else refuses the mode instead of weakening it: a managed setting or a plugin that
//! changes an agent after the configuration, or a server whose agents differ from the probe's.
//!
//! Also refused: an agent list that is empty or not a list of agents with ids, an id listed twice,
//! and rules that cannot be read.

use serde_json::{Map, Value, json};

use super::modes::{Rule, mode_rules, never_asked};
use crate::model::ApprovalMode;

/// Why an agent's rules are not the probe's followed by the mode's.
const CHANGED_AFTER: &str = "a managed setting or a plugin changes";

/// The configuration that gives every agent of the `GET /api/agent` list `agents` the rules of
/// `mode`; an error names what cannot be expressed.
pub fn mode_config(mode: ApprovalMode, agents: &Value) -> Result<Value, String> {
    let mut configured = Map::new();

    for (id, rules) in listed_agents(agents)? {
        let list = mode_rules(Some(mode), &rules).unwrap_or_default();
        configured.insert(id, json!({ "permissions": list }));
    }

    Ok(json!({ "agents": configured }))
}

/// The rules `config` (a [`mode_config`]) gives agent `id`.
pub fn configured_rules(config: &Value, id: &str) -> Vec<Rule> {
    let rules = config["agents"][id]["permissions"].clone();

    serde_json::from_value(rules).unwrap_or_default()
}

/// The agents of a `GET /api/agent` list (its `data`) with their rules; an error unless it is a
/// non-empty list of agents with an id each, listed once, and readable rules.
pub fn listed_agents(agents: &Value) -> Result<Vec<(String, Vec<Rule>)>, String> {
    let listed = agents
        .as_array()
        .filter(|listed| !listed.is_empty())
        .ok_or_else(|| "OpenCode listed no agents".to_owned())?;
    let mut read: Vec<(String, Vec<Rule>)> = Vec::new();

    for agent in listed {
        let id = agent
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| "OpenCode listed an agent without an id".to_owned())?;

        if read.iter().any(|(seen, _)| seen == id) {
            return Err(format!("two agents have the id {id}"));
        }

        let rules = agent
            .get("permissions")
            .cloned()
            .and_then(|rules| serde_json::from_value(rules).ok())
            .ok_or_else(|| format!("the {id} agent's permissions cannot be read"))?;
        read.push((id.to_owned(), rules));
    }

    Ok(read)
}

/// The inherited configuration `base` with the mode's `config` added: each agent's rules after the
/// inherited ones (see the module docs).
pub fn merge_content(base: &Value, config: &Value) -> Result<Value, String> {
    let mut merged = base.as_object().cloned().unwrap_or_default();
    let configured = config["agents"].as_object().cloned().unwrap_or_default();

    for (id, ours) in configured {
        let legacy = ["agent", "mode"].into_iter().find(|key| base[*key].get(&id).is_some());

        if let Some(key) = legacy {
            return Err(format!(
                "it configures the {id} agent the OpenCode 1 way (`{key}.{id}`), which OpenCode 2 replaces"
            ));
        }

        let agents = merged
            .entry("agents")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| "its `agents` is not an object".to_owned())?;
        let agent = agents
            .entry(id.clone())
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| format!("its {id} agent is not an object"))?;
        let rules = agent
            .entry("permissions")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| format!("its {id} agent's permissions are not a list"))?;

        rules.extend(ours["permissions"].as_array().cloned().unwrap_or_default());
    }

    Ok(Value::Object(merged))
}

/// Checks that every agent the server reports with the mode (`applied`) decides as the probe's
/// (`probed`) with its mode rules from `config` (see the module docs). An error names the first
/// agent that does not.
pub fn check_applied(probed: &Value, applied: &Value, config: &Value) -> Result<(), String> {
    let before = listed_agents(probed)?;
    let after = listed_agents(applied)?;

    let ids = |agents: &[(String, Vec<Rule>)]| {
        let mut ids: Vec<String> = agents.iter().map(|(id, _)| id.clone()).collect();
        ids.sort();
        ids
    };

    if ids(&before) != ids(&after) {
        return Err("the agents differ between the two starts".to_owned());
    }

    for (id, rules) in &after {
        let probe = before
            .iter()
            .find(|(seen, _)| seen == id)
            .map(|(_, rules)| rules.as_slice())
            .unwrap_or_default();
        let ours = configured_rules(config, id);

        if ours.is_empty() || !applied_after(probe, &ours, rules) {
            return Err(format!("{CHANGED_AFTER} the {id} agent's permissions"));
        }
    }

    Ok(())
}

/// True when `after` is the probe's rules, `ours`, then denies (see the module docs).
fn applied_after(probe: &[Rule], ours: &[Rule], after: &[Rule]) -> bool {
    let Some(at) = after.windows(ours.len()).rposition(|window| window == ours) else {
        return false;
    };

    let (prefix, rest) = after.split_at(at);
    let tail = &rest[ours.len()..];

    if !tail.iter().all(|rule| rule.is("deny")) {
        return false;
    }

    let (probe_core, probe_denies) = trailing_denies(probe);
    let (prefix_core, prefix_denies) = trailing_denies(prefix);

    if decisions(probe_core) != decisions(prefix_core) {
        return false;
    }

    prefix_denies
        .iter()
        .all(|deny| probe_denies.contains(deny) || tail.contains(deny))
}

/// `rules` split before the run of denies they end with.
fn trailing_denies(rules: &[Rule]) -> (&[Rule], &[Rule]) {
    let core = rules
        .iter()
        .rposition(|rule| !rule.is("deny"))
        .map_or(0, |last| last + 1);

    rules.split_at(core)
}

/// `rules` with each run of rules of one effect sorted: OpenCode applies the last rule that
/// matches, so the order inside such a run decides nothing.
fn decisions(rules: &[Rule]) -> Vec<Rule> {
    let mut sorted: Vec<Rule> = Vec::with_capacity(rules.len());

    for run in rules.chunk_by(|a, b| a.effect == b.effect) {
        let mut run = run.to_vec();
        run.sort_by(|a, b| (&a.action, &a.resource).cmp(&(&b.action, &b.resource)));
        sorted.extend(run);
    }

    sorted
}

/// True when a session's own rules (`permissions` of `GET /api/session/{id}`) hold anything but
/// denies: OpenCode applies them after the agent's rules, so they would override the mode.
pub fn stored_overrides(permissions: &Value) -> bool {
    match permissions {
        Value::Null => false,
        Value::Array(rules) => rules
            .iter()
            .any(|rule| rule.get("effect").and_then(Value::as_str) != Some("deny")),
        _ => true,
    }
}

/// The actions of the project's saved "Always allow" rules (`GET /api/permission/saved` `data`)
/// that `mode` would ask for: OpenCode 2 applies saved rules after every agent's, so they let
/// such calls through without asking. Empty for Yolo, which asks for nothing.
pub fn saved_overrides(mode: ApprovalMode, saved: &Value) -> Vec<String> {
    let Some(exempt) = never_asked(mode) else {
        return Vec::new();
    };

    let mut actions: Vec<String> = Vec::new();
    let listed = saved.as_array().into_iter().flatten();

    for action in listed.filter_map(|rule| rule.get("action").and_then(Value::as_str)) {
        if !exempt.contains(&action) && !actions.iter().any(|seen| seen == action) {
            actions.push(action.to_owned());
        }
    }

    actions
}
