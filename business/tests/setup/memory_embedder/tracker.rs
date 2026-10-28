//! The lines the embedder prints folded into one state by the tracker.

use harness_business::ecosystem::{EmbedderEvent, EmbedderTracker, parse_event};
use harness_business::model::{EmbedderPhase, MemoryEmbedderState};
use pretty_assertions::assert_eq;
use serde_json::json;

fn tracker_after(lines: &[&str]) -> EmbedderTracker {
    let mut tracker = EmbedderTracker::new();
    tracker.starting();

    for line in lines {
        tracker.apply(parse_event(line).unwrap_or_else(|| panic!("an event: {line}")));
    }

    tracker
}

fn shown(tracker: &EmbedderTracker) -> (EmbedderPhase, Option<f64>) {
    (tracker.state().phase, tracker.state().progress)
}

#[test]
fn every_line_the_embedder_prints_is_an_event_and_anything_else_is_nothing() {
    let cases = [
        (
            r#"{"event":"download","received":50,"total":200}"#,
            EmbedderEvent::Download {
                received: 50,
                total: 200,
            },
        ),
        (r#"{"event":"verifying"}"#, EmbedderEvent::Verifying),
        (r#"{"event":"loading"}"#, EmbedderEvent::Loading),
        (
            r#"{"event":"ready","model":"m1","dim":384}"#,
            EmbedderEvent::Ready { model: "m1".to_owned() },
        ),
        (
            r#"{"event":"backfill","store":"local:ab","done":3,"total":9}"#,
            EmbedderEvent::Backfill {
                store: "local:ab".to_owned(),
                done: 3,
                total: 9,
            },
        ),
        (
            r#"{"event":"model-update","from":"m1","to":"m2"}"#,
            EmbedderEvent::ModelUpdate,
        ),
        (
            r#"{"event":"reembed","store":"local:ab","done":1,"total":2}"#,
            EmbedderEvent::Reembed {
                store: "local:ab".to_owned(),
                done: 1,
                total: 2,
            },
        ),
        (
            r#"{"event":"switched","model":"m2"}"#,
            EmbedderEvent::Switched { model: "m2".to_owned() },
        ),
        (r#"{"event":"cleanup-done"}"#, EmbedderEvent::CleanupDone),
        (r#"{"event":"idle"}"#, EmbedderEvent::Idle),
        (
            r#"{"event":"error","message":"no disk"}"#,
            EmbedderEvent::Error {
                message: "no disk".to_owned(),
            },
        ),
        (r#"{"event":"busy"}"#, EmbedderEvent::Busy),
    ];

    for (line, event) in cases {
        assert_eq!(parse_event(line), Some(event), "{line}");
    }

    for line in [
        "",
        "not json",
        r#"{"event":"from-the-future"}"#,
        r#"{"no":"event"}"#,
        r#"[1]"#,
    ] {
        assert_eq!(parse_event(line), None, "{line:?}");
    }
}

#[test]
fn the_download_the_load_and_the_backfill_add_up_to_one_state() {
    let mut tracker = EmbedderTracker::new();
    assert_eq!(tracker.state(), &MemoryEmbedderState::default());
    assert!(tracker.starting());
    assert_eq!(shown(&tracker), (EmbedderPhase::Starting, None));

    let step = |tracker: &mut EmbedderTracker, line: &str| {
        let changed = tracker.apply(parse_event(line).unwrap());
        (changed, shown(tracker))
    };

    assert_eq!(
        step(&mut tracker, r#"{"event":"download","received":1,"total":4}"#),
        (true, (EmbedderPhase::Downloading, Some(0.25)))
    );
    assert_eq!(
        step(&mut tracker, r#"{"event":"download","received":1,"total":4}"#),
        (false, (EmbedderPhase::Downloading, Some(0.25))),
        "the same state is not a change"
    );
    assert_eq!(
        step(&mut tracker, r#"{"event":"download","received":2,"total":0}"#),
        (true, (EmbedderPhase::Downloading, None)),
        "no total, no fraction"
    );
    assert_eq!(
        step(&mut tracker, r#"{"event":"verifying"}"#),
        (true, (EmbedderPhase::Verifying, None))
    );
    assert_eq!(
        step(&mut tracker, r#"{"event":"loading"}"#),
        (true, (EmbedderPhase::Loading, None))
    );
    assert_eq!(
        step(&mut tracker, r#"{"event":"ready","model":"m1","dim":384}"#),
        (true, (EmbedderPhase::Ready, None))
    );
    assert_eq!(tracker.state().model.as_deref(), Some("m1"));

    // Two stores count together, and the embedder is ready again once both are through.
    assert_eq!(
        step(&mut tracker, r#"{"event":"backfill","store":"a","done":0,"total":6}"#),
        (true, (EmbedderPhase::Backfilling, Some(0.0)))
    );
    assert_eq!(
        step(&mut tracker, r#"{"event":"backfill","store":"b","done":2,"total":2}"#),
        (true, (EmbedderPhase::Backfilling, Some(0.25)))
    );
    assert_eq!(
        step(&mut tracker, r#"{"event":"backfill","store":"a","done":6,"total":6}"#),
        (true, (EmbedderPhase::Ready, None))
    );
}

#[test]
fn a_newer_model_keeps_the_embedder_updating_until_it_switched() {
    let mut tracker = tracker_after(&[
        r#"{"event":"ready","model":"m1","dim":384}"#,
        r#"{"event":"model-update","from":"m1","to":"m2"}"#,
        r#"{"event":"download","received":1,"total":2}"#,
    ]);
    assert_eq!(shown(&tracker), (EmbedderPhase::Updating, Some(0.5)));

    tracker.apply(parse_event(r#"{"event":"reembed","store":"a","done":1,"total":4}"#).unwrap());
    assert_eq!(shown(&tracker), (EmbedderPhase::Updating, Some(0.25)));
    assert_eq!(
        tracker.state().model.as_deref(),
        Some("m1"),
        "the old model serves meanwhile"
    );

    tracker.apply(parse_event(r#"{"event":"switched","model":"m2"}"#).unwrap());
    assert_eq!(shown(&tracker), (EmbedderPhase::Ready, None));
    assert_eq!(tracker.state().model.as_deref(), Some("m2"));

    // A failed move leaves the old model serving, and says why.
    tracker.apply(parse_event(r#"{"event":"model-update","from":"m2","to":"m3"}"#).unwrap());
    tracker.apply(parse_event(r#"{"event":"error","message":"checksum mismatch"}"#).unwrap());
    assert_eq!(shown(&tracker), (EmbedderPhase::Ready, None));
    assert_eq!(tracker.state().error.as_deref(), Some("checksum mismatch"));
}

#[test]
fn an_error_fails_the_embedder_and_busy_hands_it_to_another_instance() {
    let failed = tracker_after(&[
        r#"{"event":"loading"}"#,
        r#"{"event":"error","message":"model is corrupt"}"#,
    ]);
    assert_eq!(shown(&failed), (EmbedderPhase::Failed, None));
    assert_eq!(failed.state().error.as_deref(), Some("model is corrupt"));
    assert!(!failed.is_busy());

    let elsewhere = tracker_after(&[r#"{"event":"busy"}"#]);
    assert_eq!(shown(&elsewhere), (EmbedderPhase::Elsewhere, None));
    assert!(elsewhere.is_busy());

    let mut tracker = tracker_after(&[r#"{"event":"ready","model":"m1","dim":384}"#]);
    assert!(tracker.stopped());
    assert_eq!(
        tracker.state(),
        &MemoryEmbedderState {
            model_present: Some(true),
            ..MemoryEmbedderState::default()
        },
        "off forgets the model it served but not that it is downloaded"
    );
}

#[test]
fn whether_a_model_is_downloaded_is_unknown_until_told_and_ready_makes_it_true() {
    let mut tracker = EmbedderTracker::new();
    assert_eq!(tracker.state().model_present, None);

    assert!(tracker.model_present(Some(false)));
    assert!(!tracker.model_present(Some(false)), "the same answer is not a change");
    assert!(tracker.starting());
    assert_eq!(tracker.state().model_present, Some(false), "starting keeps it");

    tracker.apply(parse_event(r#"{"event":"ready","model":"m1","dim":384}"#).unwrap());
    assert_eq!(tracker.state().model_present, Some(true));

    assert!(tracker.failed("gone"));
    assert_eq!(tracker.state().model_present, Some(true), "a failure keeps it");

    assert!(tracker.model_present(Some(false)), "a removal says it is gone");
    assert_eq!(tracker.state().model_present, Some(false));
}

#[test]
fn an_error_message_is_redacted_before_it_is_shown() {
    let secret = ["ghp_", &"a1B2".repeat(9)].concat();
    let line = json!({ "event": "error", "message": format!("cannot reach https://u:{secret}@host/x") }).to_string();

    let tracker = tracker_after(&[&line]);

    let message = tracker.state().error.clone().unwrap();
    assert!(!message.contains(&secret), "{message}");
}

#[test]
fn stores_that_report_the_largest_counts_neither_overflow_nor_stay_unfinished() {
    let huge = u64::MAX;
    let lines = [
        json!({ "event": "backfill", "store": "a", "done": huge, "total": huge }).to_string(),
        json!({ "event": "backfill", "store": "b", "done": huge, "total": huge }).to_string(),
    ];

    let tracker = tracker_after(&[&lines[0], &lines[1]]);

    assert_eq!(shown(&tracker), (EmbedderPhase::Ready, None));
}

#[test]
fn a_model_name_from_the_embedder_is_redacted_and_clipped_like_an_error() {
    let secret = ["ghp_", &"a1B2".repeat(9)].concat();
    let long = "m".repeat(300);

    for event in ["ready", "switched"] {
        let line = json!({ "event": event, "model": format!("{long}{secret}") }).to_string();
        let model = tracker_after(&[&line]).state().model.clone().expect("a model");

        assert_eq!(model.chars().count(), 200, "{event}");
        assert!(!model.contains(&secret), "{event}");

        let line = json!({ "event": event, "model": format!("cache/{secret}") }).to_string();
        let model = tracker_after(&[&line]).state().model.clone().expect("a model");

        assert!(model.starts_with("cache/"), "{event}: {model}");
        assert!(!model.contains(&secret), "{event}: {model}");

        // A secret that straddles the clip is masked whole, not cut into an unmasked prefix.
        let line = json!({ "event": event, "model": format!("{}/{secret}", "m".repeat(195)) }).to_string();
        let model = tracker_after(&[&line]).state().model.clone().expect("a model");

        assert!(model.chars().count() <= 200, "{event}");
        assert!(!model.contains("ghp_"), "{event}: {model}");
    }
}
