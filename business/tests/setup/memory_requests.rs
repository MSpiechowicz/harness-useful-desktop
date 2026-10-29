//! Script run requests of Harness Memory: the data folder found as the server finds it, the
//! heartbeat file, the requests the watcher shows (within the size and age limits), the answers
//! written back for shown requests only, and stale answers swept.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use harness_business::BusinessError;
use harness_business::agent::rfc3339;
use harness_business::ecosystem::memory_requests::{
    ALIVE_FILE, MAX_REQUEST_AGE, MAX_REQUEST_BYTES, REQUESTS_DIR, RequestWatcher, is_request_id, memory_data_dir,
    remove_alive, touch_alive,
};
use harness_business::env::{Env, OsKind};
use harness_business::model::{
    ScriptApproval, ScriptPreviousOrigin, ScriptPreviousVersion, ScriptRequestEvent, ScriptScope, ScriptStdin,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tempfile::TempDir;

const ID: &str = "0123456789abcdef0123456789abcdef";
const OTHER_ID: &str = "fedcba9876543210fedcba9876543210";

fn summary() -> Value {
    json!({
        "id": 12, "name": "open-prs", "description": "Open pull requests", "language": "bash",
        "scope": "repo", "repo": "github.com/o/r", "sha256": "ab".repeat(32), "timeoutSecs": 60,
        "place": "team", "author": { "source": "codex", "root": null },
        "createdAt": "2026-10-01T08:00:00Z", "updatedAt": "2026-10-17T09:00:00Z", "runCount": 3,
        "lastRunAt": null, "lastExitCode": null, "trusted": false
    })
}

fn request(id: &str) -> Value {
    json!({
        "version": 1, "requestId": id, "createdAt": "2026-10-17T10:00:00Z", "client": "claude-code",
        "script": summary(), "body": "gh pr list\n", "language": "bash", "args": ["--limit", "5"],
        "previous": {
            "body": "gh pr list --state open\n", "language": "bash", "trustedAt": "2026-10-02T09:00:00Z",
            "origin": "saved"
        }
    })
}

struct Data {
    _dir: TempDir,
    root: PathBuf,
}

impl Data {
    /// A data folder with a private `requests/` in it, as the server creates them.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("harness-memory");
        std::fs::create_dir_all(root.join(REQUESTS_DIR)).unwrap();
        private(&root.join(REQUESTS_DIR));

        Self { _dir: dir, root }
    }

    fn requests(&self) -> PathBuf {
        self.root.join(REQUESTS_DIR)
    }

    fn write(&self, id: &str, content: &str) -> PathBuf {
        let path = self.requests().join(format!("{id}.json"));
        std::fs::write(&path, content).unwrap();
        path
    }
}

/// Makes `dir` private to the user (Unix), as the server creates its folders.
pub(crate) fn private(dir: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

fn age(path: &Path, by: Duration) {
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - by).unwrap();
}

fn ids(events: &[ScriptRequestEvent]) -> Vec<String> {
    events
        .iter()
        .map(|event| match event {
            ScriptRequestEvent::Request { request } => format!("request {}", request.request_id),
            ScriptRequestEvent::Gone { request_id } => format!("gone {request_id}"),
        })
        .collect()
}

#[test]
fn the_data_folder_is_found_as_the_server_finds_it() {
    let linux = Env::for_tests("/home/u").with_os(OsKind::Linux);
    let folder = |env: &Env| memory_data_dir(env);

    assert_eq!(
        folder(&linux),
        Some(PathBuf::from("/home/u/.local/share/harness-memory"))
    );
    assert_eq!(
        folder(&linux.clone().with_var("XDG_DATA_HOME", "/data")),
        Some(PathBuf::from("/data/harness-memory"))
    );
    assert_eq!(
        folder(&linux.clone().with_var("XDG_DATA_HOME", "relative")),
        Some(PathBuf::from("/home/u/.local/share/harness-memory")),
        "a relative XDG_DATA_HOME is ignored"
    );

    // The override names the folder itself, without `harness-memory` appended.
    assert_eq!(
        folder(&linux.clone().with_var("HARNESS_MEMORY_DIR", "/srv/memory")),
        Some(PathBuf::from("/srv/memory"))
    );
    assert_eq!(
        folder(&linux.clone().with_var("HARNESS_MEMORY_DIR", "~/memory")),
        Some(PathBuf::from("/home/u").join("memory"))
    );
    for ignored in ["memory", ""] {
        assert_eq!(
            folder(&linux.clone().with_var("HARNESS_MEMORY_DIR", ignored)),
            Some(PathBuf::from("/home/u/.local/share/harness-memory")),
            "{ignored:?} is ignored"
        );
    }

    let mac = Env::for_tests("/Users/u").with_os(OsKind::MacOs);
    assert_eq!(
        folder(&mac),
        Some(PathBuf::from("/Users/u/Library/Application Support/harness-memory"))
    );

    let windows = Env::for_tests("/Users/u").with_os(OsKind::Windows);
    assert_eq!(folder(&windows), None);
    assert_eq!(folder(&windows.clone().with_var("LOCALAPPDATA", "AppData")), None);
    assert_eq!(
        folder(&windows.with_var("LOCALAPPDATA", "/Users/u/AppData/Local")),
        Some(PathBuf::from("/Users/u/AppData/Local/harness-memory"))
    );
}

#[test]
fn the_heartbeat_is_touched_only_inside_an_existing_folder_and_removed() {
    let data = Data::new();
    let alive = data.root.join(ALIVE_FILE);

    let missing = data.root.join("not-yet");
    assert!(!touch_alive(&missing).unwrap());
    assert!(!missing.exists(), "the server creates the folder");

    assert!(touch_alive(&data.root).unwrap());
    assert!(alive.is_file());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&alive).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    age(&alive, Duration::from_secs(600));
    assert!(touch_alive(&data.root).unwrap());
    let modified = std::fs::metadata(&alive).unwrap().modified().unwrap();
    assert!(
        SystemTime::now().duration_since(modified).unwrap_or_default() < Duration::from_secs(60),
        "a touch makes the file fresh"
    );

    remove_alive(&data.root).unwrap();
    assert!(!alive.exists());
    remove_alive(&data.root).expect("removing a missing file is fine");
}

#[cfg(unix)]
#[test]
fn the_heartbeat_is_not_written_through_a_symbolic_link() {
    let data = Data::new();
    let target = data.root.join("elsewhere");
    std::os::unix::fs::symlink(&target, data.root.join(ALIVE_FILE)).unwrap();

    let result = touch_alive(&data.root);
    assert!(matches!(result, Err(BusinessError::InvalidInput(_))), "{result:?}");
    assert!(!target.exists());
}

#[test]
fn a_new_request_is_shown_once_and_reported_gone_when_its_file_goes() {
    let data = Data::new();
    let mut watcher = RequestWatcher::new(&data.root);
    let path = data.write(ID, &request(ID).to_string());

    // Seen a minute after the server dated it, by this app's clock.
    let seen = SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_224_060);
    let file = std::fs::File::options().write(true).open(&path).unwrap();
    file.set_modified(seen).unwrap();

    let events = watcher.poll(seen);
    let [ScriptRequestEvent::Request { request }] = events.as_slice() else {
        panic!("expected one request, got {events:?}");
    };
    assert_eq!(request.request_id, ID);
    assert_eq!(request.created_at, "2026-10-17T10:00:00Z", "the server's clock");
    assert_eq!(request.received_at, rfc3339(seen), "this app's clock");
    assert_eq!(request.client, "claude-code");
    assert_eq!(request.script.name, "open-prs");
    assert_eq!(request.script.approval, ScriptApproval::Needed, "read from trusted");
    assert_eq!(request.body, "gh pr list\n");
    assert_eq!(request.args, ["--limit", "5"]);
    assert_eq!(
        request.previous,
        Some(ScriptPreviousVersion {
            body: "gh pr list --state open\n".to_owned(),
            language: "bash".to_owned(),
            trusted_at: "2026-10-02T09:00:00Z".to_owned(),
            name: None,
            scope: None,
            origin: ScriptPreviousOrigin::Saved,
        })
    );
    assert_eq!(request.cwd, "");
    assert_eq!(request.stdin, None);

    assert!(watcher.poll(seen).is_empty(), "shown once");
    assert_eq!(
        watcher.pending().len(),
        1,
        "still waiting, for a page that starts listening"
    );

    std::fs::remove_file(path).unwrap();
    assert_eq!(ids(&watcher.poll(seen)), [format!("gone {ID}")]);
    assert!(watcher.pending().is_empty());
    assert!(watcher.poll(seen).is_empty());
}

#[test]
fn a_previous_version_without_origin_was_approved() {
    let data = Data::new();
    let mut watcher = RequestWatcher::new(&data.root);

    let mut older = request(ID);
    older["previous"].as_object_mut().unwrap().remove("origin");
    data.write(ID, &older.to_string());

    let events = watcher.poll(SystemTime::now());
    let [ScriptRequestEvent::Request { request }] = events.as_slice() else {
        panic!("expected one request, got {events:?}");
    };
    assert_eq!(
        request.previous.as_ref().map(|previous| previous.origin),
        Some(ScriptPreviousOrigin::Approved)
    );
}

#[test]
fn the_folder_the_input_and_the_old_name_and_scope_are_read_when_the_server_names_them() {
    let data = Data::new();
    let mut watcher = RequestWatcher::new(&data.root);

    let mut full = request(ID);
    full["cwd"] = json!("/work/app");
    full["stdin"] = json!({ "preview": "token=1\n", "bytes": 4096 });
    full["previous"]["name"] = json!("list-prs");
    full["previous"]["scope"] = json!("global");
    data.write(ID, &full.to_string());

    let events = watcher.poll(SystemTime::now());
    let [ScriptRequestEvent::Request { request }] = events.as_slice() else {
        panic!("expected one request, got {events:?}");
    };
    let previous = request.previous.as_ref().unwrap();

    assert_eq!(request.cwd, "/work/app");
    assert_eq!(
        request.stdin,
        Some(ScriptStdin {
            preview: "token=1\n".to_owned(),
            bytes: 4096
        })
    );
    assert_eq!(previous.name.as_deref(), Some("list-prs"));
    assert_eq!(previous.scope, Some(ScriptScope::Global));
}

#[test]
fn a_request_without_client_or_previous_is_still_shown() {
    let data = Data::new();
    let mut watcher = RequestWatcher::new(&data.root);

    let mut minimal = request(ID);
    minimal["stdin"] = Value::Null;
    let fields = minimal.as_object_mut().unwrap();
    fields.remove("client");
    fields.remove("previous");
    fields.remove("args");
    data.write(ID, &minimal.to_string());

    let events = watcher.poll(SystemTime::now());
    let [ScriptRequestEvent::Request { request }] = events.as_slice() else {
        panic!("expected one request, got {events:?}");
    };
    assert_eq!(request.client, "");
    assert_eq!(request.previous, None);
    assert!(request.args.is_empty());
}

#[test]
fn requests_outside_the_limits_are_ignored() {
    let data = Data::new();
    let mut watcher = RequestWatcher::new(&data.root);

    let mut other_version = request(OTHER_ID);
    other_version["version"] = json!(2);
    data.write(OTHER_ID, &other_version.to_string());

    let mismatched = "11111111111111111111111111111111";
    data.write(mismatched, &request(ID).to_string());

    let broken = "22222222222222222222222222222222";
    data.write(broken, "{ not json");

    let large = "33333333333333333333333333333333";
    let mut padded = request(large);
    padded["body"] = json!("x".repeat(usize::try_from(MAX_REQUEST_BYTES).unwrap()));
    data.write(large, &padded.to_string());

    let old = "44444444444444444444444444444444";
    let old_path = data.write(old, &request(old).to_string());
    age(&old_path, MAX_REQUEST_AGE + Duration::from_secs(5));

    // Names that are not request ids.
    std::fs::write(
        data.requests().join("ABCDEF0123456789ABCDEF0123456789.json"),
        request(ID).to_string(),
    )
    .unwrap();
    std::fs::write(data.requests().join(format!("{ID}.decision")), "allow").unwrap();
    std::fs::write(data.requests().join("short.json"), "{}").unwrap();

    assert!(watcher.poll(SystemTime::now()).is_empty());
    assert!(watcher.pending().is_empty());
}

#[test]
fn a_waiting_request_that_grows_too_old_is_gone() {
    let data = Data::new();
    let mut watcher = RequestWatcher::new(&data.root);
    data.write(ID, &request(ID).to_string());

    assert_eq!(ids(&watcher.poll(SystemTime::now())), [format!("request {ID}")]);

    let later = SystemTime::now() + MAX_REQUEST_AGE + Duration::from_secs(1);
    assert_eq!(ids(&watcher.poll(later)), [format!("gone {ID}")]);
    assert!(watcher.poll(later).is_empty(), "not shown again while its file stays");
}

#[test]
fn several_requests_come_oldest_first_and_without_a_folder_nothing_does() {
    let data = Data::new();
    let mut watcher = RequestWatcher::new(&data.root);

    let first = data.write(OTHER_ID, &request(OTHER_ID).to_string());
    age(&first, Duration::from_secs(10));
    data.write(ID, &request(ID).to_string());

    assert_eq!(
        ids(&watcher.poll(SystemTime::now())),
        [format!("request {OTHER_ID}"), format!("request {ID}")]
    );

    let empty = tempfile::tempdir().unwrap();
    assert!(RequestWatcher::new(empty.path()).poll(SystemTime::now()).is_empty());
}

#[cfg(unix)]
#[test]
fn a_requests_folder_other_users_may_change_is_not_read() {
    use std::os::unix::fs::PermissionsExt;

    let data = Data::new();
    let mut watcher = RequestWatcher::new(&data.root);
    data.write(ID, &request(ID).to_string());
    std::fs::set_permissions(data.requests(), std::fs::Permissions::from_mode(0o777)).unwrap();

    assert!(watcher.poll(SystemTime::now()).is_empty());
}

#[test]
fn an_answer_is_written_in_one_rename_for_a_shown_request_only() {
    let data = Data::new();
    let mut watcher = RequestWatcher::new(&data.root);
    let decision = data.requests().join(format!("{ID}.decision"));

    for bad in [
        "",
        "0123",
        &ID.to_uppercase(),
        &format!("{ID}0"),
        "../../../../../../etc/passwd00",
    ] {
        let result = watcher.decide(bad, true);
        assert!(
            matches!(result, Err(BusinessError::InvalidInput(_))),
            "{bad:?}: {result:?}"
        );
    }
    assert!(!is_request_id("0123456789abcdef0123456789abcdeg"));

    let missing = watcher.decide(ID, true);
    assert!(matches!(missing, Err(BusinessError::NotFound(_))), "{missing:?}");

    // The file alone is not enough: the user must have been shown the request.
    data.write(ID, &request(ID).to_string());
    let unseen = watcher.decide(ID, true);
    assert!(matches!(unseen, Err(BusinessError::NotFound(_))), "{unseen:?}");
    assert!(!decision.exists());

    assert_eq!(ids(&watcher.poll(SystemTime::now())), [format!("request {ID}")]);
    watcher.decide(ID, true).unwrap();
    assert_eq!(std::fs::read_to_string(&decision).unwrap(), "allow");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&decision).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    let mut names: Vec<String> = std::fs::read_dir(data.requests())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [format!("{ID}.decision"), format!("{ID}.json")],
        "no temporary file is left"
    );

    // Answered once: a second answer cannot change it, and the request is not shown again.
    let again = watcher.decide(ID, false);
    assert!(matches!(again, Err(BusinessError::NotFound(_))), "{again:?}");
    assert_eq!(std::fs::read_to_string(&decision).unwrap(), "allow");
    assert!(watcher.pending().is_empty());
    assert!(watcher.poll(SystemTime::now()).is_empty());
}

#[test]
fn a_shown_request_whose_file_is_gone_cannot_be_answered() {
    let data = Data::new();
    let mut watcher = RequestWatcher::new(&data.root);
    let path = data.write(ID, &request(ID).to_string());
    watcher.poll(SystemTime::now());

    std::fs::remove_file(path).unwrap();

    let gone = watcher.decide(ID, true);
    assert!(matches!(gone, Err(BusinessError::NotFound(_))), "{gone:?}");
    assert!(!data.requests().join(format!("{ID}.decision")).exists());
}

#[test]
fn answers_left_without_their_request_are_swept_once_stale() {
    let data = Data::new();
    let mut watcher = RequestWatcher::new(&data.root);

    let orphan = data.requests().join(format!("{ID}.decision"));
    std::fs::write(&orphan, "allow").unwrap();

    let waiting = data.requests().join(format!("{OTHER_ID}.decision"));
    std::fs::write(&waiting, "deny").unwrap();
    data.write(OTHER_ID, &request(OTHER_ID).to_string());
    age(&waiting, MAX_REQUEST_AGE + Duration::from_secs(5));

    let not_an_answer = data.requests().join("notes.decision");
    std::fs::write(&not_an_answer, "allow").unwrap();
    age(&not_an_answer, MAX_REQUEST_AGE + Duration::from_secs(5));

    watcher.poll(SystemTime::now());
    assert!(orphan.exists(), "a fresh answer may still be read by the server");

    age(&orphan, MAX_REQUEST_AGE + Duration::from_secs(5));
    watcher.poll(SystemTime::now());
    assert!(!orphan.exists(), "a stale answer without its request is removed");
    assert!(waiting.exists(), "an answer whose request is still there stays");
    assert!(not_an_answer.exists(), "other names are left alone");
}
