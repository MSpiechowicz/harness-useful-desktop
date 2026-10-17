//! The MCP server mode (`harness-desktop mcp`) driven over in-memory stdin and stdout: the
//! handshake, the tool list and its annotations, and agents adding, listing, changing and
//! removing hooks under the agent rules (pending until approved, the sha256 precondition, removal
//! only of hooks an agent created), with `project` defaulting to the server's working folder.

mod hooks_mcp {
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::time::SystemTime;

    use harness_business::hooks::{APPROVALS_FILE, HookCapabilities, HookStore};
    use harness_business::hooks_mcp::{Server, serve};
    use harness_business::model::{HookAction, HookAuthor, HookDraft, HookEvent, HookScope};
    use pretty_assertions::assert_eq;
    use serde_json::{Value, json};
    use tempfile::TempDir;

    /// A temporary machine: the hooks folder, the app's config folder, and a repository with a
    /// subfolder the server is started in.
    struct Folder {
        temp: TempDir,
        store: HookStore,
        config: PathBuf,
        repo: PathBuf,
    }

    fn folder() -> Folder {
        let temp = tempfile::tempdir().unwrap();
        let base = dunce::canonicalize(temp.path()).unwrap();

        let repo = base.join("repo");
        for folder in [".git", "src/deep"] {
            std::fs::create_dir_all(repo.join(folder)).unwrap();
        }

        let config = base.join("app-config");

        Folder {
            store: HookStore::new(base.join("hooks-root"), &config),
            config,
            repo,
            temp,
        }
    }

    impl Folder {
        fn server(&self) -> Server {
            Server::new(Some(self.store.clone()), Some(self.repo.join("src/deep")))
        }

        /// Serves `messages` (one per line) and returns every answer.
        fn exchange(&self, messages: &[Value]) -> Vec<Value> {
            exchange_with(&mut self.server(), messages)
        }

        /// The result of one `tools/call`.
        fn call(&self, tool: &str, arguments: Value) -> Value {
            let answers = self.exchange(&[call(1, tool, arguments)]);
            answers[0]["result"].clone()
        }

        fn repo_root(&self) -> String {
            self.repo.to_string_lossy().into_owned()
        }
    }

    fn exchange_with(server: &mut Server, messages: &[Value]) -> Vec<Value> {
        let input: String = messages.iter().map(|message| format!("{message}\n")).collect();
        exchange_raw(server, input.as_bytes())
    }

    fn exchange_raw(server: &mut Server, input: &[u8]) -> Vec<Value> {
        let mut output = Vec::new();
        serve(Cursor::new(input.to_vec()), &mut output, server).unwrap();

        String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn call(id: u64, tool: &str, arguments: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": tool, "arguments": arguments } })
    }

    fn text_of(result: &Value) -> &str {
        result["content"][0]["text"].as_str().unwrap()
    }

    fn add_args() -> Value {
        json!({
            "name": "Run the tests",
            "event": "turnEnd",
            "action": "followUp",
            "text": "Run cargo test before you finish.",
            "scope": "project",
        })
    }

    fn desktop_hook(store: &HookStore) -> (String, String) {
        let draft = HookDraft {
            id: None,
            expected_sha256: None,
            name: "Answer style".to_owned(),
            description: None,
            scope: HookScope::Global,
            project: None,
            event: HookEvent::PromptSubmit,
            matcher: None,
            action: HookAction::AddContext {
                text: "Short answers.".to_owned(),
            },
            enabled: true,
        };

        let view = store
            .save(
                &draft,
                HookAuthor::Desktop,
                HookCapabilities::default(),
                SystemTime::now(),
            )
            .unwrap();
        (view.id, view.sha256)
    }

    #[test]
    fn initialize_negotiates_the_version_and_names_the_server() {
        let folder = folder();
        let initialize = |version: &str| {
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                "protocolVersion": version, "capabilities": {}, "clientInfo": { "name": "omp", "version": "1" }
            } })
        };

        let answers = folder.exchange(&[
            initialize("2025-06-18"),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            initialize("1999-01-01"),
            json!({ "jsonrpc": "2.0", "id": "p", "method": "ping" }),
        ]);

        assert_eq!(answers.len(), 3, "the notification is not answered");

        let result = &answers[0]["result"];
        assert_eq!(result["protocolVersion"], "2025-06-18");
        assert_eq!(result["serverInfo"]["name"], "harness-desktop");
        assert_eq!(result["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(result["capabilities"], json!({ "tools": { "listChanged": false } }));
        assert!(result["instructions"].as_str().unwrap().contains("Hooks view"));

        assert_eq!(
            answers[1]["result"]["protocolVersion"], "2025-11-25",
            "an unknown version gets the newest"
        );
        assert_eq!(answers[2], json!({ "jsonrpc": "2.0", "id": "p", "result": {} }));
    }

    #[test]
    fn tools_list_carries_the_annotations_of_each_tool() {
        let folder = folder();
        let answers = folder.exchange(&[json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })]);
        let tools = answers[0]["result"]["tools"].as_array().unwrap();

        let hints: Vec<(String, Value, Value)> = tools
            .iter()
            .map(|tool| {
                (
                    tool["name"].as_str().unwrap().to_owned(),
                    tool["annotations"]["readOnlyHint"].clone(),
                    tool["annotations"]["destructiveHint"].clone(),
                )
            })
            .collect();

        assert_eq!(
            hints,
            vec![
                ("hook_list".to_owned(), json!(true), json!(false)),
                ("hook_get".to_owned(), json!(true), json!(false)),
                ("hook_add".to_owned(), json!(false), json!(false)),
                ("hook_update".to_owned(), json!(false), json!(false)),
                ("hook_remove".to_owned(), json!(false), json!(true)),
            ]
        );

        for tool in tools {
            assert_eq!(tool["inputSchema"]["additionalProperties"], json!(false));
            assert_eq!(tool["annotations"]["openWorldHint"], json!(false));
        }
    }

    #[test]
    fn an_added_hook_is_listed_as_pending_in_the_project_of_the_working_folder() {
        let folder = folder();

        let added = folder.call("hook_add", add_args());
        assert_eq!(added["isError"], json!(false), "{}", text_of(&added));

        let hook = &added["structuredContent"]["added"];
        assert_eq!(hook["status"], "pending");
        assert_eq!(hook["approval"], "pending");
        assert_eq!(
            hook["project"],
            json!(folder.repo_root()),
            "the repository root of the working folder"
        );
        assert_eq!(hook["createdBy"], "agent");
        assert_eq!(
            hook["action"],
            json!({ "type": "followUp", "text": "Run cargo test before you finish." })
        );
        assert_eq!(
            serde_json::from_str::<Value>(text_of(&added)).unwrap(),
            added["structuredContent"],
            "the text carries the same reply"
        );

        let listed = folder.call("hook_list", json!({}));
        let reply = &listed["structuredContent"];
        assert_eq!(reply["project"], json!(folder.repo_root()));
        assert_eq!(reply["projectHooks"], "1 of 64");
        assert_eq!(reply["hooks"].as_array().unwrap().len(), 1);
        assert_eq!(reply["hooks"][0]["id"], hook["id"]);
        assert_eq!(reply["hooks"][0]["status"], "pending");
        assert!(reply["note"].as_str().unwrap().contains("approves"));

        let global_only = folder.call("hook_list", json!({ "scope": "global" }));
        assert_eq!(global_only["structuredContent"]["hooks"], json!([]));

        assert!(
            !folder.config.join(APPROVALS_FILE).exists(),
            "the server never writes the approvals"
        );
    }

    #[test]
    fn a_hook_of_another_project_is_not_listed_here() {
        let folder = folder();
        let other = folder.temp.path().join("other");
        std::fs::create_dir_all(&other).unwrap();

        let mut args = add_args();
        args["project"] = json!(other.to_string_lossy());
        assert_eq!(folder.call("hook_add", args)["isError"], json!(false));

        let here = folder.call("hook_list", json!({}));
        assert_eq!(here["structuredContent"]["hooks"], json!([]));

        let there = folder.call("hook_list", json!({ "project": other.to_string_lossy() }));
        assert_eq!(there["structuredContent"]["hooks"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn an_approved_desktop_hook_is_listed_as_approved() {
        let folder = folder();
        let (id, sha256) = desktop_hook(&folder.store);

        let got = folder.call("hook_get", json!({ "id": id }));
        let hook = &got["structuredContent"]["hook"];

        assert_eq!(hook["status"], "active");
        assert_eq!(hook["approval"], "approved");
        assert_eq!(hook["sha256"], json!(sha256));
        assert_eq!(hook["scope"], "global");
    }

    #[test]
    fn an_update_with_a_stale_sha256_is_refused() {
        let folder = folder();
        let added = folder.call("hook_add", add_args());
        let id = added["structuredContent"]["added"]["id"].clone();
        let stale = added["structuredContent"]["added"]["sha256"].clone();

        let updated = folder.call(
            "hook_update",
            json!({ "id": id, "sha256": stale, "text": "Run all tests." }),
        );
        assert_eq!(updated["isError"], json!(false), "{}", text_of(&updated));
        let hook = &updated["structuredContent"]["updated"];
        assert_eq!(hook["action"]["text"], "Run all tests.");
        assert_eq!(hook["name"], "Run the tests", "fields left out are kept");
        assert_eq!(hook["status"], "pending");

        let refused = folder.call("hook_update", json!({ "id": id, "sha256": stale, "enabled": false }));
        assert_eq!(refused["isError"], json!(true));
        assert!(
            text_of(&refused).contains("changed since you read it"),
            "{}",
            text_of(&refused)
        );
    }

    #[test]
    fn an_agent_edit_of_a_user_hook_makes_it_pending() {
        let folder = folder();
        let (id, sha256) = desktop_hook(&folder.store);

        let updated = folder.call(
            "hook_update",
            json!({ "id": id, "sha256": sha256, "text": "Tiny answers." }),
        );
        let hook = &updated["structuredContent"]["updated"];

        assert_eq!(hook["status"], "pending");
        assert_eq!(hook["createdBy"], "desktop");
        assert_eq!(hook["updatedBy"], "agent");
    }

    #[test]
    fn an_agent_does_not_move_a_user_hook_to_another_scope_or_project() {
        let folder = folder();
        let (id, sha256) = desktop_hook(&folder.store);
        let other = folder.temp.path().join("other");
        std::fs::create_dir_all(&other).unwrap();

        for change in [
            json!({ "scope": "project" }),
            json!({ "scope": "project", "project": other.to_string_lossy() }),
        ] {
            let mut arguments = json!({ "id": id, "sha256": sha256 });
            arguments
                .as_object_mut()
                .unwrap()
                .extend(change.as_object().unwrap().clone());

            let refused = folder.call("hook_update", arguments);
            assert_eq!(refused["isError"], json!(true), "{change}");
            assert!(text_of(&refused).contains("ask the user"), "{}", text_of(&refused));
        }

        let kept = folder.call("hook_get", json!({ "id": id }));
        assert_eq!(kept["structuredContent"]["hook"]["sha256"], json!(sha256));

        // A hook an agent created may still move, and stays pending.
        let added = folder.call("hook_add", add_args());
        let hook = &added["structuredContent"]["added"];
        let moved = folder.call(
            "hook_update",
            json!({ "id": hook["id"], "sha256": hook["sha256"], "scope": "global" }),
        );
        assert_eq!(moved["isError"], json!(false), "{}", text_of(&moved));
        assert_eq!(moved["structuredContent"]["updated"]["scope"], "global");
        assert_eq!(moved["structuredContent"]["updated"]["status"], "pending");
    }

    #[test]
    fn secrets_in_a_hook_are_not_shown_to_the_agent() {
        let folder = folder();
        let token = format!("{}_{}", "npm", "a1B2".repeat(8));
        let mut arguments = add_args();
        arguments["action"] = json!("command");
        arguments["event"] = json!("sessionEnd");
        arguments["command"] = json!(format!("curl -H 'token: {token}' https://example.invalid"));
        arguments.as_object_mut().unwrap().remove("text");

        let added = folder.call("hook_add", arguments);
        let id = added["structuredContent"]["added"]["id"].clone();

        for shown in [
            folder.call("hook_get", json!({ "id": id })),
            folder.call("hook_list", json!({})),
            added,
        ] {
            let text = text_of(&shown).to_owned();
            assert!(!text.contains(&token), "{text}");
            assert!(text.contains("curl -H"), "{text}");
        }

        let file =
            std::fs::read_to_string(folder.store.hooks_dir().join(format!("{}.json", id.as_str().unwrap()))).unwrap();
        assert!(file.contains(&token), "the file keeps what the agent wrote");
    }

    #[test]
    fn removing_a_hook_the_desktop_created_is_refused() {
        let folder = folder();
        let (id, sha256) = desktop_hook(&folder.store);

        let refused = folder.call("hook_remove", json!({ "id": id, "sha256": sha256 }));

        assert_eq!(refused["isError"], json!(true));
        assert!(text_of(&refused).contains("ask them"), "{}", text_of(&refused));
        assert_eq!(
            folder.call("hook_get", json!({ "id": id }))["isError"],
            json!(false),
            "the hook is kept"
        );
    }

    #[test]
    fn an_agent_removes_the_hook_it_created() {
        let folder = folder();
        let added = folder.call("hook_add", add_args());
        let hook = &added["structuredContent"]["added"];

        let removed = folder.call("hook_remove", json!({ "id": hook["id"], "sha256": hook["sha256"] }));
        assert_eq!(removed["isError"], json!(false), "{}", text_of(&removed));

        let gone = folder.call("hook_get", json!({ "id": hook["id"] }));
        assert_eq!(gone["isError"], json!(true));
        assert_eq!(text_of(&gone), "no hook has this id");
    }

    #[test]
    fn invalid_combinations_and_arguments_are_refused() {
        let folder = folder();
        let refuse = |arguments: Value| {
            let result = folder.call("hook_add", arguments);
            assert_eq!(result["isError"], json!(true));
            text_of(&result).to_owned()
        };

        let mut wrong_event = add_args();
        wrong_event["action"] = json!("addContext");
        assert!(refuse(wrong_event).contains("turnEnd"));

        let mut text_on_command = add_args();
        text_on_command["action"] = json!("command");
        assert!(refuse(text_on_command).contains("give command"));

        let mut unknown = add_args();
        unknown["timeout"] = json!(5);
        assert!(refuse(unknown).contains("unknown field"));

        let mut global_with_project = add_args();
        global_with_project["scope"] = json!("global");
        global_with_project["project"] = json!(folder.repo_root());
        assert!(refuse(global_with_project).contains("scope project"));

        let mut missing_project = add_args();
        missing_project["project"] = json!(folder.temp.path().join("nowhere").to_string_lossy());
        assert!(refuse(missing_project).contains("does not exist"));

        assert!(
            folder.store.list(HookCapabilities::default()).hooks.is_empty(),
            "nothing was written"
        );
    }

    #[test]
    fn protocol_faults_are_answered_and_serving_goes_on() {
        let folder = folder();
        let mut server = folder.server();
        let input = b"not json\n[1]\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"nope\"}\n\n{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"hook_nope\"}}\n{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"ping\"}\n";

        let codes: Vec<Value> = exchange_raw(&mut server, input)
            .iter()
            .map(|answer| answer["error"]["code"].clone())
            .collect();

        assert_eq!(
            codes,
            vec![json!(-32700), json!(-32600), json!(-32601), json!(-32602), Value::Null]
        );
    }

    #[test]
    fn without_a_hooks_folder_every_tool_refuses() {
        let mut server = Server::new(None, None);
        let answers = exchange_with(&mut server, &[call(1, "hook_list", json!({}))]);

        assert_eq!(answers[0]["result"]["isError"], json!(true));
        assert!(text_of(&answers[0]["result"]).contains("not available"));
    }
}
