//! The plan limits a prompt carries: which windows and budgets are close, whose plan a chat sees,
//! and the levels a chat is told once.

use harness_business::ecosystem::{
    DashboardReading, Gauge, LimitLevels, close_gauges, close_limits, limit_providers, limit_steer,
};
use harness_business::model::HarnessId;
use jiff::Timestamp;
use jiff::tz::TimeZone;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn now() -> Timestamp {
    "2026-11-03T12:00:00Z".parse().unwrap()
}

fn window(provider: &str, window: &str, percent: Value) -> Value {
    json!({
        "provider": provider,
        "plan": "max",
        "window": window,
        "scope": null,
        "usedPercent": percent,
        "resetsAt": "2026-11-03T14:00:00Z",
        "ageMinutes": 3,
        "status": "fresh",
    })
}

fn reading(windows: Vec<Value>, budgets: Vec<Value>) -> DashboardReading {
    DashboardReading {
        limits: json!({ "windows": windows }),
        budgets: json!({ "budgets": budgets }),
    }
}

/// The gauges of a Claude 5 hour window used `percent`.
fn claude_gauges_at(percent: f64) -> Vec<Gauge> {
    close_gauges(
        &reading(vec![window("claude", "5h", json!(percent))], Vec::new()),
        &["claude".to_owned()],
        None,
    )
}

fn lines(reading: &DashboardReading, providers: &[String]) -> Vec<String> {
    lines_in(reading, providers, None)
}

fn lines_in(reading: &DashboardReading, providers: &[String], project: Option<&str>) -> Vec<String> {
    let gauges = close_gauges(reading, providers, project);

    close_limits(&gauges, now(), &TimeZone::UTC)
        .map(|text| text.lines().map(str::to_owned).collect())
        .unwrap_or_default()
}

#[test]
fn only_windows_used_80_percent_or_more_are_close_fullest_first_and_at_most_three_lines() {
    let reading = reading(
        vec![
            window("claude", "5h", json!(86)),
            window("claude", "7d", json!(79.9)),
            window("claude", "1d", json!(80)),
            window("codex", "3d", json!(97)),
            window("opencode-go", "30d", json!(91)),
            window("claude", "5h", json!(null)),
        ],
        Vec::new(),
    );

    let every = ["claude", "codex", "opencode-go"].map(str::to_owned);

    assert_eq!(
        lines(&reading, &every),
        [
            "Close to a limit: Codex 3-day window 97% used, resets 14:00 UTC.",
            "Close to a limit: OpenCode Go 30-day window 91% used, resets 14:00 UTC.",
            "Close to a limit: Claude 5-hour window 86% used, resets 14:00 UTC.",
        ]
    );
}

#[test]
fn nothing_close_says_nothing() {
    let reading = reading(vec![window("claude", "5h", json!(40))], Vec::new());

    assert_eq!(lines(&reading, &["claude".to_owned()]), Vec::<String>::new());
    assert_eq!(close_limits(&[], now(), &TimeZone::UTC), None);
}

#[test]
fn a_chat_sees_the_windows_of_its_own_provider_and_other_harnesses_see_all() {
    let reading = reading(
        vec![window("claude", "5h", json!(86)), window("codex", "5h", json!(92))],
        vec![json!({ "scope": "global", "period": "monthly", "project": null, "capUsd": 200, "usedPercent": 81 })],
    );

    assert_eq!(
        lines(&reading, &limit_providers(HarnessId::Claude, None)),
        [
            "Close to a limit: Claude 5-hour window 86% used, resets 14:00 UTC.",
            "Close to a limit: monthly budget ($200) 81% used.",
        ]
    );
    assert_eq!(
        lines(&reading, &limit_providers(HarnessId::Codex, None)),
        [
            "Close to a limit: Codex 5-hour window 92% used, resets 14:00 UTC.",
            "Close to a limit: monthly budget ($200) 81% used.",
        ]
    );
    // An OMP chat on an unknown plan sees only the budget, never another plan's window.
    assert_eq!(
        lines(&reading, &limit_providers(HarnessId::Omp, None)),
        ["Close to a limit: monthly budget ($200) 81% used."]
    );
}

#[test]
fn a_chat_sees_the_windows_of_the_provider_of_its_current_model() {
    let reading = reading(
        vec![
            window("claude", "7d", json!(87)),
            window("opencode-go", "Monthly limit", json!(91)),
        ],
        vec![],
    );

    let omp_on_opencode_go = lines(&reading, &limit_providers(HarnessId::Omp, Some("opencode-go")));
    assert_eq!(omp_on_opencode_go.len(), 1);
    assert!(omp_on_opencode_go[0].contains("91%"), "{omp_on_opencode_go:?}");

    let omp_on_anthropic = lines(&reading, &limit_providers(HarnessId::Omp, Some("anthropic")));
    assert_eq!(omp_on_anthropic.len(), 1);
    assert!(omp_on_anthropic[0].contains("87%"), "{omp_on_anthropic:?}");
}

#[test]
fn a_stale_window_or_one_that_reset_says_nothing_and_a_far_reset_names_its_day() {
    let mut stale = window("claude", "5h", json!(90));
    stale["status"] = json!("stale");
    let mut reset = window("claude", "5h", json!(90));
    reset["status"] = json!("reset since reading");
    let mut weekly = window("claude", "7d", json!(90));
    weekly["scope"] = json!("Fable");
    weekly["resetsAt"] = json!("2026-11-05T09:00:00Z");
    let mut past = window("claude", "1d", json!(85));
    past["resetsAt"] = json!("2026-11-03T11:00:00Z");

    assert_eq!(
        lines(
            &reading(vec![stale, reset, weekly, past], Vec::new()),
            &["claude".to_owned()]
        ),
        [
            "Close to a limit: Claude weekly window (Fable) 90% used, resets Thu 09:00 UTC.",
            "Close to a limit: Claude daily window 85% used.",
        ]
    );
}

#[test]
fn a_level_is_told_once_at_80_and_once_at_95() {
    let mut levels = LimitLevels::default();
    let at = claude_gauges_at;

    assert_eq!(levels.crossed(&at(60.0)).len(), 0, "not close yet");
    assert_eq!(levels.crossed(&at(82.0)).len(), 1, "80 crossed");
    assert_eq!(levels.crossed(&at(88.0)).len(), 0, "the same level again");
    assert_eq!(levels.crossed(&at(95.0)).len(), 1, "95 crossed");
    assert_eq!(levels.crossed(&at(99.0)).len(), 0, "the same level again");
    assert_eq!(
        levels.crossed(&at(82.0)).len(),
        0,
        "never repeated after a reset either"
    );
}

#[test]
fn what_the_prompt_showed_is_not_news_until_the_next_level() {
    let mut levels = LimitLevels::default();
    let at = claude_gauges_at;

    levels.told(&at(86.0));

    assert_eq!(levels.crossed(&at(90.0)).len(), 0);
    assert_eq!(levels.crossed(&at(96.0)).len(), 1);
}

#[test]
fn the_steer_names_what_crossed_and_what_to_do() {
    let gauges = close_gauges(
        &reading(vec![window("claude", "5h", json!(91))], Vec::new()),
        &["claude".to_owned()],
        None,
    );

    assert_eq!(
        limit_steer(&gauges, now(), &TimeZone::UTC).as_deref(),
        Some(
            "Limit close: Claude 5-hour window 91% used, resets 14:00 UTC. Spawn fewer agents, or tell the user \
             before you continue."
        )
    );
    assert_eq!(limit_steer(&[], now(), &TimeZone::UTC), None);
}

fn budget(project: Value, percent: u32) -> Value {
    json!({ "scope": "project", "period": "monthly", "project": project, "capUsd": 100, "usedPercent": percent })
}

#[test]
fn a_chat_sees_the_budgets_of_no_project_and_of_its_own_project_only() {
    let reading = reading(
        Vec::new(),
        vec![
            budget(json!(null), 81),
            budget(json!("/work/app"), 92),
            budget(json!("/work/other"), 99),
            budget(json!("/elsewhere/app"), 98),
        ],
    );

    assert_eq!(
        lines_in(&reading, &[], Some("/work/app")),
        [
            "Close to a limit: monthly budget ($100) of /work/app 92% used.",
            "Close to a limit: monthly budget ($100) 81% used.",
        ]
    );
}

#[test]
fn a_chat_without_a_project_sees_no_budget_of_a_project() {
    let reading = reading(
        Vec::new(),
        vec![budget(json!(null), 81), budget(json!("/work/app"), 92)],
    );

    assert_eq!(
        lines_in(&reading, &[], None),
        ["Close to a limit: monthly budget ($100) 81% used."]
    );
}

#[test]
fn a_budgets_project_matches_by_folder_ignoring_separators_and_a_trailing_one_and_by_a_bare_name() {
    let reading = reading(
        Vec::new(),
        vec![
            budget(json!("C:\\work\\app\\"), 90),
            budget(json!("app"), 85),
            budget(json!("apps"), 99),
        ],
    );

    assert_eq!(lines_in(&reading, &[], Some("C:/work/app")).len(), 2);
    assert_eq!(
        lines_in(&reading, &[], Some("/home/me/app")).len(),
        1,
        "only the bare name"
    );
    assert_eq!(lines_in(&reading, &[], Some("/home/me/other")).len(), 0);
}
