//! The plan limit settings in `PrefsStore`: a missing or bad value loads its default, and the file is kept.

use std::fs;

use harness_business::model::{LimitBarsPref, LimitGaugePref, ThemePref};
use harness_business::prefs::{PREFS_FILE, PrefsOrigin, PrefsStore};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::prefs::{file_names, load_file};

#[test]
fn a_file_without_the_limit_settings_loads_their_defaults() {
    let loaded = load_file(r#"{ "version": 1, "theme": "dark" }"#);

    assert_eq!(loaded.limit_bars, LimitBarsPref::Used);
    assert!(loaded.limit_forecast);
    assert_eq!(loaded.limit_gauge, LimitGaugePref::Bars);
}

#[test]
fn stored_limit_settings_are_kept() {
    let loaded = load_file(r#"{ "limitBars": "left", "limitForecast": false, "limitGauge": "waffle" }"#);

    assert_eq!(loaded.limit_bars, LimitBarsPref::Left);
    assert!(!loaded.limit_forecast);
    assert_eq!(loaded.limit_gauge, LimitGaugePref::Waffle);
}

#[test]
fn bad_limit_settings_load_their_defaults_and_keep_the_file() {
    let dir = TempDir::new().unwrap();
    let store = PrefsStore::new(dir.path().to_path_buf());

    for bad in [
        "null",
        "3",
        "[]",
        r#"{ "mode": "left" }"#,
        "true",
        r#""sideways""#,
        r#""LEFT""#,
    ] {
        let text = format!(
            r#"{{ "version": 1, "theme": "dark", "limitBars": {bad}, "limitForecast": {bad}, "limitGauge": {bad} }}"#
        );
        fs::write(store.path(), text).unwrap();

        let (loaded, origin) = store.load_checked();

        assert_eq!(origin, PrefsOrigin::Stored, "{bad}");
        assert_eq!(loaded.theme, ThemePref::Dark, "{bad}");
        assert_eq!(loaded.limit_bars, LimitBarsPref::Used, "{bad}");
        assert_eq!(loaded.limit_gauge, LimitGaugePref::Bars, "{bad}");
        // Only a real `false` hides the forecast.
        assert!(loaded.limit_forecast, "{bad}");
        assert_eq!(file_names(dir.path()), vec![PREFS_FILE.to_owned()], "{bad}");
    }
}
