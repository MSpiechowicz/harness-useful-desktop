//! The routing of an alert, what the notification says, the chime throttle, clicks and the bounds on
//! what the page reports.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use harness_business::model::AppPrefs;
use serde_json::{Value, json};

use super::view::{MAX_BODY_CHARS, MAX_KEY_BYTES, MAX_TITLE_CHARS, MAX_TITLES, bounded, notification_body};
use super::*;
use crate::test_support::{closable_channel, collecting_channel, wait_until};

const KEY: &str = "0f4c8a1e-7d52-4c1b-9a3e-5b6d7e8f9a0b";
const OTHER: &str = "9b8a7c6d-1234-4f5e-8a9b-0c1d2e3f4a5b";

/// A host that records what an alert did.
struct FakeHost {
    prefs: AppPrefs,
    in_front: bool,
    calls: Mutex<Vec<String>>,
}

impl FakeHost {
    fn new(in_front: bool) -> Self {
        Self {
            prefs: AppPrefs::default(),
            in_front,
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn record(&self, call: String) {
        self.calls.lock().unwrap().push(call);
    }
}

/// Lets a test keep the host it attached.
struct Attached(Arc<FakeHost>);

impl Host for Attached {
    fn prefs(&self) -> AppPrefs {
        self.0.prefs()
    }

    fn in_front(&self) -> bool {
        self.0.in_front()
    }

    fn attention(&self) {
        self.0.attention();
    }

    fn notify(&self, title: &str, body: &str, key: &str) {
        self.0.notify(title, body, key);
    }

    fn chime(&self) {
        self.0.chime();
    }
}

impl Host for FakeHost {
    fn prefs(&self) -> AppPrefs {
        self.prefs.clone()
    }

    fn in_front(&self) -> bool {
        self.in_front
    }

    fn attention(&self) {
        self.record("attention".to_owned());
    }

    fn notify(&self, title: &str, body: &str, key: &str) {
        self.record(format!("notify {title} | {body} | {key}"));
    }

    fn chime(&self) {
        self.record("chime".to_owned());
    }
}

fn view_of(shown: Option<&str>, titles: &[(&str, &str)]) -> ChatAlertsView {
    ChatAlertsView {
        shown: shown.map(str::to_owned),
        titles: titles
            .iter()
            .map(|(key, title)| ((*key).to_owned(), (*title).to_owned()))
            .collect(),
    }
}

/// Connects a page to `alerts` that shows the chat `shown`, and returns what the page receives.
fn listening_with_view(alerts: &Shared, shown: Option<&str>) -> Arc<Mutex<Vec<Value>>> {
    let (channel, messages) = collecting_channel::<ChatAlertEvent>();

    *alerts.listener.lock().unwrap() = Some(channel);
    *alerts.view.lock().unwrap() = view_of(shown, &[]);

    messages
}

fn kinds_of(messages: &Mutex<Vec<Value>>) -> Vec<Value> {
    messages.lock().unwrap().clone()
}

#[test]
fn an_alert_goes_where_the_user_is_not_looking() {
    // (enabled, in front, shown chat) -> what happens to the alert of KEY.
    let table = [
        (false, false, None, Route::Silent),
        (false, true, Some(OTHER), Route::Silent),
        (true, false, Some(KEY), Route::Notify),
        (true, false, None, Route::Notify),
        (true, true, Some(KEY), Route::Silent),
        (true, true, Some(OTHER), Route::Toast),
        (true, true, None, Route::Toast),
    ];

    for (enabled, in_front, shown, expected) in table {
        assert_eq!(
            route(enabled, in_front, shown, KEY),
            expected,
            "{enabled} {in_front} {shown:?}"
        );
    }
}

#[test]
fn a_chat_that_is_not_in_front_notifies_with_its_name_and_chimes() {
    let alerts = Shared::default();
    let host = FakeHost::new(false);
    alerts
        .view
        .lock()
        .unwrap()
        .titles
        .insert(KEY.to_owned(), "Fix the login bug".to_owned());

    alerts.deliver(&host, KEY, ChatAlertKind::Finished, Instant::now());

    assert_eq!(
        host.calls(),
        [
            "attention".to_owned(),
            format!("notify Chat finished | Fix the login bug | {KEY}"),
            "chime".to_owned(),
        ]
    );
}

#[test]
fn the_notification_title_follows_the_kind() {
    for (kind, title) in [
        (ChatAlertKind::Finished, "Chat finished"),
        (ChatAlertKind::NeedsYou, "Chat needs you"),
        (ChatAlertKind::Failed, "Chat failed"),
    ] {
        let host = FakeHost::new(false);

        Shared::default().deliver(&host, KEY, kind, Instant::now());

        assert!(host.calls()[1].starts_with(&format!("notify {title} | ")), "{kind:?}");
    }
}

#[test]
fn a_chat_without_a_usable_name_gets_the_generic_body() {
    for title in [None, Some(""), Some("  \n\t "), Some("<>&")] {
        let alerts = Shared::default();
        let host = FakeHost::new(false);

        if let Some(title) = title {
            alerts
                .view
                .lock()
                .unwrap()
                .titles
                .insert(KEY.to_owned(), title.to_owned());
        }
        alerts.deliver(&host, KEY, ChatAlertKind::Failed, Instant::now());

        assert_eq!(
            host.calls()[1],
            format!("notify Chat failed | Open it in Harness Desktop | {KEY}"),
            "{title:?}"
        );
    }
}

#[test]
fn a_chat_on_another_view_gets_a_toast_and_a_chime_but_no_notification() {
    let alerts = Shared::default();
    let host = FakeHost::new(true);
    let messages = listening_with_view(&alerts, Some(OTHER));

    alerts.deliver(&host, KEY, ChatAlertKind::NeedsYou, Instant::now());

    assert_eq!(host.calls(), ["chime"]);
    assert_eq!(
        kinds_of(&messages),
        [json!({ "type": "toast", "key": KEY, "kind": "needsYou" })]
    );
}

#[test]
fn the_chat_on_screen_is_silent() {
    let alerts = Shared::default();
    let host = FakeHost::new(true);
    let messages = listening_with_view(&alerts, Some(KEY));

    alerts.deliver(&host, KEY, ChatAlertKind::Finished, Instant::now());

    assert!(host.calls().is_empty(), "no chime: {:?}", host.calls());
    assert!(kinds_of(&messages).is_empty());
}

#[test]
fn switched_off_alerts_do_nothing_at_all() {
    for in_front in [false, true] {
        let alerts = Shared::default();
        let mut host = FakeHost::new(in_front);
        host.prefs.chat_alerts = false;
        let (channel, messages) = collecting_channel::<ChatAlertEvent>();
        *alerts.listener.lock().unwrap() = Some(channel);

        alerts.deliver(&host, KEY, ChatAlertKind::Failed, Instant::now());

        assert!(host.calls().is_empty(), "{in_front}");
        assert!(kinds_of(&messages).is_empty(), "{in_front}");
    }
}

#[test]
fn the_chime_has_its_own_switch() {
    let alerts = Shared::default();
    let mut host = FakeHost::new(false);
    host.prefs.alert_sound = false;

    alerts.deliver(&host, KEY, ChatAlertKind::Finished, Instant::now());

    let calls = host.calls();
    assert_eq!(calls.len(), 2, "attention and notification, no chime: {calls:?}");
    assert!(!calls.contains(&"chime".to_owned()));
}

#[test]
fn alerts_in_a_burst_chime_once_and_the_next_one_after_the_pause_chimes_again() {
    let alerts = Shared::default();
    let host = FakeHost::new(false);
    let start = Instant::now();

    alerts.deliver(&host, KEY, ChatAlertKind::Finished, start);
    alerts.deliver(
        &host,
        OTHER,
        ChatAlertKind::Finished,
        start + Duration::from_millis(400),
    );
    alerts.deliver(
        &host,
        KEY,
        ChatAlertKind::Failed,
        start + CHIME_EVERY - Duration::from_millis(1),
    );

    let chimes = |host: &FakeHost| host.calls().iter().filter(|call| *call == "chime").count();
    let notifications = |host: &FakeHost| host.calls().iter().filter(|call| call.starts_with("notify")).count();

    assert_eq!(chimes(&host), 1, "one chime for three alerts");
    assert_eq!(notifications(&host), 3, "every alert still notifies");

    alerts.deliver(&host, KEY, ChatAlertKind::Finished, start + CHIME_EVERY);

    assert_eq!(chimes(&host), 2);
}

#[test]
fn a_silent_alert_does_not_start_the_chime_pause() {
    let alerts = Shared::default();
    let start = Instant::now();
    *alerts.view.lock().unwrap() = view_of(Some(KEY), &[]);

    let on_screen = FakeHost::new(true);
    alerts.deliver(&on_screen, KEY, ChatAlertKind::Finished, start);

    let elsewhere = FakeHost::new(false);
    alerts.deliver(&elsewhere, OTHER, ChatAlertKind::Finished, start);

    assert!(elsewhere.calls().contains(&"chime".to_owned()));
}

#[test]
fn a_notification_body_drops_markup_and_control_characters() {
    assert_eq!(
        notification_body("Fix <b>the</b> login & \u{7}bug\u{0}").as_deref(),
        Some("Fix bthe/b login bug")
    );
    assert_eq!(notification_body("a\nb\t c\r\n\r\nd").as_deref(), Some("a b c d"));
    assert_eq!(
        notification_body("zero\u{200b}width \u{202e}rtl").as_deref(),
        Some("zerowidth rtl")
    );
    assert_eq!(notification_body("  padded  ").as_deref(), Some("padded"));
}

#[test]
fn a_notification_body_masks_a_secret_in_the_opening_words_of_a_first_message() {
    // Built at run time: a token-shaped literal would trip secret scanning.
    let token = format!("{}_{}", "ghp", "a1B2c3D4".repeat(3));
    let hidden = format!("{}_\u{200b}{}", "ghp", "a1B2c3D4".repeat(3));

    for title in [
        format!("Fix login with {token} please"),
        format!("Fix login with {hidden} please"),
    ] {
        let body = notification_body(&title).unwrap();

        assert!(body.starts_with("Fix login with "), "{body}");
        assert!(body.ends_with(" please"), "{body}");
        assert!(!body.contains("a1B2c3D4"), "{body}");
    }
}

#[test]
fn a_notification_body_masks_a_secret_that_dropped_markup_would_rejoin() {
    // Built at run time: a token-shaped literal would trip secret scanning.
    let half = "a1B2c3D4a1B2";

    for mark in ['<', '>', '&'] {
        let split = format!("{}_{half}{mark}{half}", "ghp");
        let body = notification_body(&format!("Fix login with {split} please")).unwrap();

        assert!(body.starts_with("Fix login with "), "{body}");
        assert!(!body.contains("a1B2c3D4"), "{mark}: {body}");
    }
}

#[test]
fn a_notification_body_has_no_text_when_nothing_usable_is_left() {
    for title in ["", "   ", "\n\t", "<>&", "\u{7}\u{200b}"] {
        assert_eq!(notification_body(title), None, "{title:?}");
    }
}

#[test]
fn a_long_notification_body_ends_in_an_ellipsis_within_the_cap() {
    let long = "word ".repeat(100);
    let body = notification_body(&long).unwrap();

    assert_eq!(body.chars().count(), MAX_BODY_CHARS);
    assert!(body.ends_with("word…"), "{body}");

    let exact = "x".repeat(MAX_BODY_CHARS);
    assert_eq!(notification_body(&exact).as_deref(), Some(exact.as_str()));

    let over = "é".repeat(MAX_BODY_CHARS + 1);
    let capped = notification_body(&over).unwrap();
    assert_eq!(capped.chars().count(), MAX_BODY_CHARS);
    assert!(capped.ends_with('…'));
}

fn clicked(key: &Value) -> HashMap<String, Value> {
    HashMap::from([(KEY_EXTRA.to_owned(), key.clone())])
}

#[test]
fn a_click_opens_only_a_chat_key_of_the_expected_form() {
    let alerts = ChatAlerts::default();
    let (channel, messages) = collecting_channel::<ChatAlertEvent>();
    alerts.listen(channel);

    let too_long = "a".repeat(MAX_KEY_BYTES + 1);
    for key in [
        json!(""),
        json!("a b"),
        json!("../x"),
        json!("a\u{e9}"),
        json!("key\n"),
        json!(too_long),
        json!(7),
        Value::Null,
    ] {
        alerts.on_notification_click(&clicked(&key));
    }
    alerts.on_notification_click(&HashMap::new());
    alerts.on_notification_click(&HashMap::from([("harnessRoutineRun".to_owned(), json!(KEY))]));

    assert!(kinds_of(&messages).is_empty(), "nothing to open");

    let longest = "a".repeat(MAX_KEY_BYTES);
    alerts.on_notification_click(&clicked(&json!(KEY)));
    alerts.on_notification_click(&clicked(&json!(longest)));

    assert_eq!(
        kinds_of(&messages),
        [
            json!({ "type": "open", "key": KEY }),
            json!({ "type": "open", "key": longest })
        ]
    );
}

#[test]
fn a_listener_whose_page_is_gone_is_dropped_and_a_new_one_replaces_the_old() {
    let alerts = ChatAlerts::default();
    let (gone, open, _) = closable_channel::<ChatAlertEvent>(true);
    alerts.listen(gone);
    open.store(false, std::sync::atomic::Ordering::Release);

    alerts.on_notification_click(&clicked(&json!(KEY)));
    assert!(alerts.shared.listener.lock().unwrap().is_none());

    let (first, first_messages) = collecting_channel::<ChatAlertEvent>();
    let (second, second_messages) = collecting_channel::<ChatAlertEvent>();
    alerts.listen(first);
    alerts.listen(second);
    alerts.on_notification_click(&clicked(&json!(KEY)));

    assert!(kinds_of(&first_messages).is_empty());
    assert_eq!(kinds_of(&second_messages).len(), 1);
}

#[test]
fn the_page_view_is_bounded_without_failing() {
    let titles: HashMap<String, String> = (0..100)
        .map(|n| (format!("chat-{n:03}"), format!("Title {n}")))
        .collect();
    let kept = bounded(ChatAlertsView {
        shown: Some("chat-007".to_owned()),
        titles,
    });

    assert_eq!(kept.titles.len(), MAX_TITLES);
    assert_eq!(kept.shown.as_deref(), Some("chat-007"));
    assert!(kept.titles.contains_key("chat-000") && kept.titles.contains_key("chat-063"));
    assert!(!kept.titles.contains_key("chat-064"));
}

#[test]
fn over_long_keys_and_titles_are_dropped_or_cut() {
    let long_key = "k".repeat(MAX_KEY_BYTES + 1);
    let edge_key = "k".repeat(MAX_KEY_BYTES);

    let kept = bounded(ChatAlertsView {
        shown: Some(long_key.clone()),
        titles: HashMap::from([
            (long_key, "dropped".to_owned()),
            (edge_key.clone(), format!("  {}  ", "t".repeat(MAX_TITLE_CHARS + 50))),
            (KEY.to_owned(), "   ".to_owned()),
            (String::new(), "no key".to_owned()),
            (OTHER.to_owned(), "  Kept  ".to_owned()),
        ]),
    });

    assert_eq!(kept.shown, None);
    assert_eq!(kept.titles.len(), 2);
    assert_eq!(kept.titles[&edge_key].chars().count(), MAX_TITLE_CHARS);
    assert_eq!(kept.titles[OTHER], "Kept");
}

#[test]
fn the_stored_view_is_the_bounded_one() {
    let alerts = ChatAlerts::default();
    alerts.set_view(view_of(Some(KEY), &[(KEY, "  Name  ")]));

    let (shown, title) = alerts.shared.view_of(KEY);
    assert_eq!(shown.as_deref(), Some(KEY));
    assert_eq!(title.as_deref(), Some("Name"));

    alerts.set_view(ChatAlertsView::default());
    assert_eq!(alerts.shared.view_of(KEY), (None, None));
}

#[test]
fn a_raised_alert_is_delivered_off_the_calling_thread_once_the_app_is_attached() {
    let alerts = ChatAlerts::default();
    let host = Arc::new(FakeHost::new(false));

    alerts.raise(OTHER, ChatAlertKind::Finished);
    alerts.attach_host(Arc::new(Attached(host.clone())));
    alerts.raise(KEY, ChatAlertKind::Failed);

    wait_until("the notification", || host.calls().len() == 3);

    assert!(host.calls()[1].ends_with(&format!("| {KEY}")), "{:?}", host.calls());
    assert_eq!(alerts.raised().len(), 2);
}
