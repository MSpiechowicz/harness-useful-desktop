//! The plan limits and budgets that are close, for the Harness Dashboard step of a prompt and for
//! the check when an agent starts a subagent.
//!
//! [`read_limits`] asks the dashboard for `limits` and `budget_status` (shaped like
//! `ui/src/lib/dashboard/models.ts` reads them: `windows[]` with `provider`, `window`, `scope`,
//! `usedPercent`, `resetsAt`, `status`, and `budgets[]` with `scope`, `period`, `project`,
//! `capUsd`, `usedPercent`) within [`LIMITS_TIMEOUT`]. [`close_gauges`] picks what is used
//! [`CLOSE_PERCENT`] or more, [`close_limits`] writes the lines of the step, and [`LimitLevels`] tells
//! which of them newly crossed a level ([`CLOSE_PERCENT`], then [`NEAR_PERCENT`]) since the chat was
//! last told. Everything but the reading is pure.
//!
//! A window with no `usedPercent` (its reading no longer applies), a stale one or one that has
//! reset since the reading says nothing. A chat of a harness whose subscription the data names
//! ([`providers_of`]) sees that provider's windows only, other harnesses see every window; budgets
//! have no provider: a budget of no project always shows, one of a project only in a chat of that project.

use std::collections::HashMap;
use std::time::Duration;

use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};
use serde_json::Value;

use super::dashboard::Dashboard;
use crate::env::Env;
use crate::model::{DashboardQuery, DashboardTool, HarnessId};

/// How long both queries together may take: a cold `harness-dashboard mcp` needs about 4 s, and the
/// desktop asks in the background (its cache), so a message never waits this long.
pub const LIMITS_TIMEOUT: Duration = Duration::from_secs(8);

/// From this share used a window or budget is close.
pub const CLOSE_PERCENT: f64 = 80.0;

/// From this share used it is nearly out.
pub const NEAR_PERCENT: f64 = 95.0;

/// The most lines the step (or one steer) names.
pub const MAX_LIMIT_LINES: usize = 3;

/// The longest name taken from the data (it ends up in a prompt).
const MAX_NAME_CHARS: usize = 40;

/// A reset further out than this is written with its weekday.
const SHORT_CLOCK: SignedDuration = SignedDuration::from_hours(20);

/// What the dashboard answered; `Null` for a query that failed.
#[derive(Debug, Clone, PartialEq)]
pub struct DashboardReading {
    /// The `limits` data.
    pub limits: Value,
    /// The `budget_status` data.
    pub budgets: Value,
}

/// One window or budget that is close.
#[derive(Debug, Clone, PartialEq)]
pub struct Gauge {
    /// Stable across readings (provider, plan, window and scope; scope, period and project).
    pub key: String,
    /// What it is: "Claude 5-hour window", "monthly budget ($200)".
    pub subject: String,
    /// How much is used, 0 to 100 and over.
    pub percent: f64,
    /// When the window resets.
    pub resets_at: Option<Timestamp>,
}

/// Reads the limits and budgets of `dashboard`; `None` when neither query answered in
/// [`LIMITS_TIMEOUT`] (not installed, failed, slow).
pub async fn read_limits(dashboard: &Dashboard, env: &Env) -> Option<DashboardReading> {
    let ask = |tool| async move {
        let query = DashboardQuery {
            tool,
            args: Value::Null,
        };

        match dashboard.query(env, &query).await {
            Ok(result) if !result.is_error => Some(result.data),
            Ok(_) | Err(_) => None,
        }
    };

    let both = async { tokio::join!(ask(DashboardTool::Limits), ask(DashboardTool::BudgetStatus)) };
    let (limits, budgets) = tokio::time::timeout(LIMITS_TIMEOUT, both).await.ok()?;

    if limits.is_none() && budgets.is_none() {
        return None;
    }

    Some(DashboardReading {
        limits: limits.unwrap_or(Value::Null),
        budgets: budgets.unwrap_or(Value::Null),
    })
}

/// The dashboard providers whose windows a chat sees: those of the provider of its current model
/// (`model_provider`, as the harness names it, e.g. `anthropic`, `openai`, `opencode-go`), else of
/// the harness itself (Claude Code, Codex). A chat whose plan is unknown (OMP, PI or OpenCode
/// before its model is known) sees no window: another plan's limit would mislead it. Budgets are
/// not tied to a provider and count by their project instead ([`close_gauges`]).
pub fn limit_providers(harness: HarnessId, model_provider: Option<&str>) -> Vec<String> {
    let names: &[&str] = match model_provider.map(str::to_ascii_lowercase).as_deref() {
        Some("anthropic" | "claude" | "claude-code") => &["claude", "anthropic"],
        Some("openai" | "codex" | "chatgpt") => &["codex", "openai"],
        Some("github-copilot" | "copilot") => &["copilot", "github-copilot"],
        Some(other) => return vec![other.to_owned()],
        None => match harness {
            HarnessId::Claude => &["claude", "anthropic"],
            HarnessId::Codex => &["codex", "openai"],
            HarnessId::Omp | HarnessId::Pi | HarnessId::OpenCode => &[],
        },
    };

    names.iter().map(|name| (*name).to_owned()).collect()
}

/// The windows and budgets of `reading` that are used [`CLOSE_PERCENT`] or more, fullest first.
/// `providers`: the provider names a window must have ([`limit_providers`]). `project`: the chat's
/// project folder, whose budgets count with the budgets of no project; another project's do not
/// (see [`budget_applies`]).
pub fn close_gauges(reading: &DashboardReading, providers: &[String], project: Option<&str>) -> Vec<Gauge> {
    let windows = array(&reading.limits, "windows")
        .iter()
        .filter_map(|window| window_gauge(window, providers));
    let budgets = array(&reading.budgets, "budgets")
        .iter()
        .filter(|budget| budget_applies(budget, project))
        .filter_map(budget_gauge);

    let mut gauges: Vec<Gauge> = windows
        .chain(budgets)
        .filter(|gauge| gauge.percent >= CLOSE_PERCENT)
        .collect();

    gauges.sort_by(|left, right| right.percent.total_cmp(&left.percent));
    gauges
}

/// The lines of the Harness Dashboard step: the first [`MAX_LIMIT_LINES`] of `gauges`, each
/// "Close to a limit: Claude 5-hour window 86% used, resets 14:00."; `None` when none is close.
pub fn close_limits(gauges: &[Gauge], now: Timestamp, zone: &TimeZone) -> Option<String> {
    let lines: Vec<String> = gauges
        .iter()
        .take(MAX_LIMIT_LINES)
        .map(|gauge| format!("Close to a limit: {}", gauge.describe(now, zone)))
        .collect();

    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// The steer for a chat whose agent started a subagent while `crossed` newly crossed a level.
pub fn limit_steer(crossed: &[Gauge], now: Timestamp, zone: &TimeZone) -> Option<String> {
    let described: Vec<String> = crossed
        .iter()
        .take(MAX_LIMIT_LINES)
        .map(|gauge| gauge.describe(now, zone))
        .collect();

    if described.is_empty() {
        return None;
    }

    Some(format!(
        "Limit close: {} Spawn fewer agents, or tell the user before you continue.",
        described.join(" ")
    ))
}

impl Gauge {
    /// "Claude 5-hour window 86% used, resets 14:00." (no reset when it is unknown or past).
    pub fn describe(&self, now: Timestamp, zone: &TimeZone) -> String {
        let used = format!("{} {:.0}% used", self.subject, self.percent);

        match self.resets_at.and_then(|at| clock(at, now, zone)) {
            Some(when) => format!("{used}, resets {when}."),
            None => format!("{used}."),
        }
    }

    /// The level this gauge stands at: 0, [`CLOSE_PERCENT`] or [`NEAR_PERCENT`] (as a whole number).
    fn level(&self) -> u8 {
        if self.percent >= NEAR_PERCENT {
            95
        } else if self.percent >= CLOSE_PERCENT {
            80
        } else {
            0
        }
    }
}

/// The highest level the chat was told for each gauge. A level is never told twice, also after the
/// window reset and filled up again.
#[derive(Debug, Default)]
pub struct LimitLevels {
    told: HashMap<String, u8>,
}

impl LimitLevels {
    /// The chat was shown `gauges` (the prompt's step): they are not news until they cross the next
    /// level.
    pub fn told(&mut self, gauges: &[Gauge]) {
        for gauge in gauges.iter().take(MAX_LIMIT_LINES) {
            self.raise(gauge);
        }
    }

    /// The gauges of `gauges` that crossed a level the chat was not told yet, fullest first, at most
    /// [`MAX_LIMIT_LINES`]; they count as told.
    pub fn crossed(&mut self, gauges: &[Gauge]) -> Vec<Gauge> {
        let mut news = Vec::new();

        for gauge in gauges {
            if self.raise(gauge) && news.len() < MAX_LIMIT_LINES {
                news.push(gauge.clone());
            }
        }

        news
    }

    /// Records the level of `gauge`; true when it is higher than the one told.
    fn raise(&mut self, gauge: &Gauge) -> bool {
        let level = gauge.level();
        let told = self.told.entry(gauge.key.clone()).or_insert(0);

        if level > *told {
            *told = level;
            return true;
        }

        false
    }
}

fn window_gauge(window: &Value, providers: &[String]) -> Option<Gauge> {
    let provider = text(window, "provider").unwrap_or("unknown");

    if !providers.iter().any(|name| name.eq_ignore_ascii_case(provider)) {
        return None;
    }

    let status = text(window, "status").unwrap_or("").to_ascii_lowercase();

    if status.contains("stale") || status.contains("reset") {
        return None;
    }

    let percent = window.get("usedPercent")?.as_f64()?;
    let length = text(window, "window").map(window_name).unwrap_or_default();
    let scope = text(window, "scope").map(name).filter(|scope| !scope.is_empty());

    let subject = match scope {
        Some(scope) => format!("{} {length} window ({scope})", provider_name(provider)),
        None => format!("{} {length} window", provider_name(provider)),
    };

    let key = format!(
        "window:{provider}:{}:{}:{}",
        text(window, "plan").unwrap_or(""),
        text(window, "window").unwrap_or(""),
        text(window, "scope").unwrap_or("")
    );

    Some(Gauge {
        key,
        subject: subject.replace("  ", " "),
        percent,
        resets_at: text(window, "resetsAt").and_then(|at| at.parse().ok()),
    })
}

/// Whether a budget is the chat's to see: it has no project, or its project is the chat's. The dashboard names
/// a budget's project by its folder path. A path is the chat's project when the folders are the same (separators
/// and a trailing one aside), a bare name when it is the project folder's name. A budget of another project says
/// nothing about this chat, and its cap and spend do not belong in this chat's prompt.
fn budget_applies(budget: &Value, project: Option<&str>) -> bool {
    let Some(named) = text(budget, "project").map(str::trim).filter(|named| !named.is_empty()) else {
        return true;
    };

    let Some(project) = project else {
        return false;
    };

    let named = folder_key(named);
    let project = folder_key(project);

    if named.contains('/') {
        return named == project;
    }

    project.rsplit('/').next() == Some(named.as_str())
}

/// A folder path to compare: `/` separators, no trailing one, and the case ignored where the file system does.
fn folder_key(path: &str) -> String {
    let path = path.replace('\\', "/");
    let path = path.trim_end_matches('/');

    if cfg!(windows) {
        path.to_lowercase()
    } else {
        path.to_owned()
    }
}

fn budget_gauge(budget: &Value) -> Option<Gauge> {
    let percent = budget.get("usedPercent")?.as_f64()?;
    let period = name(text(budget, "period").unwrap_or("")).to_lowercase();
    let scope = name(text(budget, "scope").unwrap_or("budget"));
    let project = text(budget, "project").map(name).filter(|project| !project.is_empty());

    let mut subject = if period.is_empty() {
        "budget".to_owned()
    } else {
        format!("{period} budget")
    };

    if let Some(cap) = budget.get("capUsd").and_then(Value::as_f64) {
        subject = format!("{subject} (${cap:.0})");
    }

    if let Some(project) = &project {
        subject = format!("{subject} of {project}");
    }

    Some(Gauge {
        key: format!("budget:{scope}:{period}:{}", project.unwrap_or_default()),
        subject,
        percent,
        resets_at: None,
    })
}

/// The window's length as a word: "5-hour", "weekly", "daily", "3-day", else its own name.
fn window_name(window: &str) -> String {
    let window = window.trim();

    let parsed = window
        .char_indices()
        .last()
        .and_then(|(index, unit)| window[..index].parse::<u32>().ok().map(|count| (count, unit)));

    match parsed {
        Some((1, 'd')) => "daily".to_owned(),
        Some((7, 'd')) => "weekly".to_owned(),
        Some((count, 'd')) if count > 0 => format!("{count}-day"),
        Some((count, 'h')) if count > 0 => format!("{count}-hour"),
        _ => name(window),
    }
}

fn provider_name(provider: &str) -> String {
    match provider.to_ascii_lowercase().as_str() {
        "claude" => "Claude".to_owned(),
        "codex" => "Codex".to_owned(),
        "copilot" => "GitHub Copilot".to_owned(),
        "google-gemini-cli" => "Gemini".to_owned(),
        "xai" => "xAI".to_owned(),
        "deepseek" => "DeepSeek".to_owned(),
        "opencode-go" => "OpenCode Go".to_owned(),
        _ => title_case(&name(provider)),
    }
}

fn title_case(text: &str) -> String {
    text.split(['-', '_', ' '])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut characters = word.chars();

            match characters.next() {
                Some(first) => first.to_uppercase().chain(characters).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

/// `text` without control characters, at most [`MAX_NAME_CHARS`] characters.
fn name(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control())
        .take(MAX_NAME_CHARS)
        .collect::<String>()
        .trim()
        .to_owned()
}

/// "14:00", with the weekday when it is further out than [`SHORT_CLOCK`]; `None` when `at` is past.
fn clock(at: Timestamp, now: Timestamp, zone: &TimeZone) -> Option<String> {
    let ahead = at.duration_since(now);

    if ahead <= SignedDuration::ZERO {
        return None;
    }

    let local = at.to_zoned(zone.clone());
    let format = if ahead > SHORT_CLOCK { "%a %H:%M %Z" } else { "%H:%M %Z" };

    Some(local.strftime(format).to_string())
}

fn array<'a>(data: &'a Value, field: &str) -> &'a [Value] {
    data.get(field).and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn text<'a>(data: &'a Value, field: &str) -> Option<&'a str> {
    data.get(field).and_then(Value::as_str)
}
