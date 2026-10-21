//! The OpenCode 2 server JSON the chat engine reads and sends (`opencode_server_v2::wire`), against
//! the recorded traffic (`fixtures/opencode2/recorded`).

mod opencode2_recorded;

use std::path::Path;

use harness_business::opencode_server_v2::MessageId;
use harness_business::opencode_server_v2::wire::{
    Agent, AgentBody, CommandBody, CompactBody, ConfigSource, CreateBody, Decision, Delivery, Envelope, FileRef,
    ForkBody, Form, FormReplyBody, FormValue, InboxEntry, Interrupted, Model, ModelBody, ModelRef, PermissionReplyBody,
    PermissionRequest, PromptBody, Provider, ServerInfo, Session, ShellBody, TitleBody,
};
use opencode2_recorded::{DIRECTORY, Exchange, exchanges, find};
use pretty_assertions::assert_eq;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn decode<T: DeserializeOwned>(exchange: &Exchange) -> T {
    let body = serde_json::to_vec(&exchange.response).expect("body");
    serde_json::from_slice(&body).unwrap_or_else(|error| panic!("{} {}: {error}", exchange.file, exchange.target))
}

fn json(body: impl Serialize) -> Value {
    serde_json::to_value(body).expect("json")
}

fn big_pickle() -> ModelRef {
    ModelRef {
        id: "big-pickle".to_owned(),
        provider_id: "opencode".to_owned(),
        variant: None,
    }
}

#[test]
fn every_recorded_response_decodes_into_its_type() {
    let mut decoded = 0;

    for exchange in exchanges() {
        let path = exchange.path();
        let method = exchange.method.as_str();

        if exchange.status == 204 {
            assert_eq!(exchange.response, Value::Null, "{} {path}", exchange.file);
            decoded += 1;
            continue;
        }

        if path == "/api/info" {
            let info: ServerInfo = decode(&exchange);
            assert_eq!(info.version, "2.1.2");
        } else if path == "/api/session" && method == "GET" {
            let list: Envelope<Vec<Session>> = decode(&exchange);
            assert!(list.next_cursor().is_some());
        } else if path.ends_with("/message") {
            let page: Envelope<Vec<Value>> = decode(&exchange);
            assert!(page.cursor.is_some(), "a message page has cursors");
        } else if path.ends_with("/permission") {
            let _: Envelope<Vec<PermissionRequest>> = decode(&exchange);
        } else if path.ends_with("/prompt") || path.ends_with("/compact") {
            let item: Envelope<InboxEntry> = decode(&exchange);
            assert!(item.data.id.starts_with("msg_"));
        } else if path.ends_with("/interrupt") {
            let _: Interrupted = decode(&exchange);
        } else if path == "/api/agent" {
            let _: Envelope<Vec<Agent>> = decode(&exchange);
        } else if path == "/api/command" {
            let _: Envelope<Vec<harness_business::opencode_server_v2::wire::Command>> = decode(&exchange);
        } else if path == "/api/model" {
            let _: Envelope<Vec<Model>> = decode(&exchange);
        } else if path == "/api/config" {
            let _: Vec<ConfigSource> = decode(&exchange);
        } else if path == "/api/form" {
            let _: Envelope<Vec<Form>> = decode(&exchange);
        } else {
            let session: Envelope<Session> = decode(&exchange);
            assert!(session.data.id.starts_with("ses_"), "{path}");
        }

        decoded += 1;
    }

    assert_eq!(decoded, exchanges().len());
}

#[test]
fn sessions_read_their_title_model_usage_and_location() {
    let created: Envelope<Session> = decode(&find("POST", "/api/session")[0]);
    let created = created.data;
    assert_eq!(created.title, None, "a new session has no title yet");
    assert_eq!(created.project_id, "global");
    assert_eq!(created.location.directory, DIRECTORY);
    assert_eq!(
        created.model,
        Some(ModelRef {
            variant: Some("default".to_owned()),
            ..big_pickle()
        })
    );
    assert_eq!(created.cost, 0.0);
    assert_eq!(created.tokens.input, 0);
    assert!(created.time.created > 1_700_000_000_000, "milliseconds");

    let used = find("GET", "")
        .into_iter()
        .filter(|exchange| exchange.path().starts_with("/api/session/") && exchange.path().matches('/').count() == 3)
        .map(|exchange| decode::<Envelope<Session>>(&exchange).data)
        .find(|session| session.tokens.input > 0)
        .expect("a session that ran");
    assert!(used.title.is_some());
    assert!(used.tokens.cache.read > 0);
    assert_eq!(used.outcome.as_deref(), Some("succeeded"));
    assert!(used.time.idle.is_some());
}

#[test]
fn forks_name_their_parent_and_boundary() {
    let forks: Vec<Session> = find("POST", "/fork")
        .iter()
        .map(|exchange| decode::<Envelope<Session>>(exchange).data)
        .collect();
    let boundaries: Vec<&str> = forks
        .iter()
        .map(|session| session.fork.as_ref().expect("fork").boundary.kind.as_str())
        .collect();

    assert_eq!(boundaries, ["before", "through"]);
    assert_eq!(
        forks[0].fork.as_ref().expect("fork").boundary.message_id.as_deref(),
        Some("msg_15d824387001C5U5EIkeFSEWKC")
    );
    assert_eq!(forks[0].title.as_deref(), Some("Recorded title (fork #1)"));
}

#[test]
fn catalogs_keep_display_fields_only() {
    let agents: Envelope<Vec<Agent>> = decode(&find("GET", "/api/agent")[0]);
    assert_eq!(
        agents.location.as_ref().map(|location| location.directory.as_str()),
        Some(DIRECTORY)
    );
    let primary: Vec<&str> = agents
        .data
        .iter()
        .filter(|agent| agent.is_primary())
        .map(|agent| agent.id.as_str())
        .collect();
    assert_eq!(primary, ["build", "plan"]);

    let commands: Envelope<Vec<harness_business::opencode_server_v2::wire::Command>> =
        decode(&find("GET", "/api/command")[0]);
    let names: Vec<&str> = commands.data.iter().map(|command| command.name.as_str()).collect();
    assert_eq!(names, ["init", "review"]);
    assert!(
        commands.data[1]
            .description
            .as_deref()
            .is_some_and(|text| text.starts_with("review"))
    );

    let models: Envelope<Vec<Model>> = decode(&find("GET", "/api/model")[0]);
    let pickle = &models.data[0];
    assert_eq!(pickle.reference(), big_pickle());
    assert_eq!(pickle.name, "Big Pickle");
    assert_eq!(pickle.limit.context, 200_000);
    assert!(pickle.variants.is_empty());
    assert!(!pickle.capabilities.takes_images());
    assert!(pickle.enabled);

    let deepseek = &models.data[1];
    let variants: Vec<&str> = deepseek.variants.iter().map(|variant| variant.id.as_str()).collect();
    assert_eq!(variants, ["low", "high", "max"]);
    assert!(deepseek.capabilities.takes_images());

    let shown = format!("{:?}", models.data);
    assert!(!shown.contains("apiKey") && !shown.contains("baseURL") && !shown.contains("inference"));

    let shown = format!("{:?}", agents.data);
    assert!(!shown.contains("headers") && !shown.contains("file search specialist"));
}

#[test]
fn providers_keep_id_name_and_activation() {
    let body = json!({
        "location": {"directory": DIRECTORY},
        "data": [{
            "id": "opencode",
            "integrationID": "opencode",
            "name": "OpenCode Zen",
            "activation": "auto",
            "package": "@opencode/ai/providers/openai-compatible",
            "settings": {"apiKey": "secret-value", "baseURL": "https://example.invalid"}
        }]
    });

    let providers: Envelope<Vec<Provider>> = serde_json::from_value(body).expect("providers");

    assert_eq!(
        providers.data[0],
        Provider {
            id: "opencode".to_owned(),
            name: "OpenCode Zen".to_owned(),
            integration_id: Some("opencode".to_owned()),
            activation: "auto".to_owned(),
        }
    );
}

#[test]
fn config_sources_read_directories_and_documents() {
    let recorded: Vec<ConfigSource> = decode(&find("GET", "/api/config")[0]);
    assert_eq!(
        recorded,
        [ConfigSource::Directory {
            path: "/home/user/.config/opencode".to_owned()
        }]
    );

    let inline = json!([
        {"type": "document", "info": {"agents": {"build": {"permissions": [{"action": "edit", "resource": "*", "effect": "ask"}]}}}},
        {"type": "remote", "url": "https://example.invalid"}
    ]);
    let sources: Vec<ConfigSource> = serde_json::from_value(inline).expect("sources");

    let ConfigSource::Document { path, info } = &sources[0] else {
        panic!("a document");
    };
    assert_eq!(*path, None);
    assert_eq!(info["agents"]["build"]["permissions"][0]["effect"], "ask");
    assert_eq!(sources[1], ConfigSource::Other);
}

#[test]
fn asks_read_permission_requests_and_forms() {
    let pending: Envelope<Vec<PermissionRequest>> = decode(&find("GET", "/permission")[0]);
    let request = &pending.data[0];
    assert_eq!(request.id, "per_15d7c3a33001lsSJ9gGeiFzCh8");
    assert_eq!(request.action, "edit");
    assert_eq!(request.resources, ["notes.txt"]);
    assert_eq!(request.save, ["*"]);
    assert_eq!(request.metadata["files"][0]["file"], "notes.txt");
    let source = request.source.as_ref().expect("tool source");
    assert_eq!(
        (source.kind.as_str(), source.id.as_str()),
        ("tool", "call_01a15d7c388975ddb41ea7f1")
    );

    let forms: Envelope<Vec<Form>> = decode(&find("GET", "/api/form")[0]);
    let form = &forms.data[0];
    assert_eq!(form.id, "frm_15d80582d001AwxHnvC4VOQ9RP");
    assert_eq!(form.metadata["kind"], "question");
    let field = &form.fields[0];
    assert_eq!((field.key.as_str(), field.kind.as_str()), ("q0", "string"));
    assert!(field.custom);
    let options: Vec<&str> = field.options.iter().map(|option| option.label.as_str()).collect();
    assert_eq!(options, ["Red", "Blue"]);
}

#[test]
fn admitted_input_is_an_inbox_item() {
    let prompt: Envelope<InboxEntry> = decode(&find("POST", "/prompt")[0]);
    assert_eq!(prompt.data.item.kind, "user");
    assert_eq!(prompt.data.item.delivery, Some(Delivery::Steer));
    assert!(prompt.data.item.payload.text.is_some());
    assert!(prompt.data.session_id.starts_with("ses_"));

    let compact: Envelope<InboxEntry> = decode(&find("POST", "/compact")[0]);
    assert_eq!(compact.data.item.kind, "compaction");
    assert_eq!(compact.data.item.payload.text, None);

    let interrupted: Interrupted = decode(&find("POST", "/interrupt")[0]);
    assert!(interrupted.interrupted);
}

#[test]
fn request_bodies_are_sent_as_recorded() {
    let body = |method: &str, suffix: &str, index: usize| find(method, suffix)[index].body.clone();

    assert_eq!(
        json(CreateBody {
            model: Some(big_pickle()),
            ..CreateBody::default()
        }),
        body("POST", "/api/session", 0)
    );

    let prompts: Vec<Value> = find("POST", "/prompt")
        .into_iter()
        .map(|exchange| exchange.body)
        .collect();
    let text = |value: &Value| value["text"].as_str().expect("text").to_owned();
    let steer = prompts.iter().find(|body| body["delivery"] == "steer").expect("steer");
    let queue = prompts.iter().find(|body| body["delivery"] == "queue").expect("queue");
    let image = prompts.iter().find(|body| body.get("files").is_some()).expect("image");

    assert_eq!(json(PromptBody::text(text(&prompts[0]))), prompts[0]);
    assert_eq!(
        json(PromptBody::text(text(steer)).with_delivery(Delivery::Steer)),
        *steer
    );
    assert_eq!(
        json(PromptBody::text(text(queue)).with_delivery(Delivery::Queue)),
        *queue
    );
    let file = FileRef::local(Path::new("/work/proj/dot.png")).expect("absolute");
    assert_eq!(json(PromptBody::text(text(image)).with_files(vec![file])), *image);

    assert_eq!(
        json(CommandBody {
            name: "review".to_owned(),
            text: String::new()
        }),
        body("POST", "/command", 0)
    );
    let shell = body("POST", "/shell", 0);
    assert_eq!(
        json(ShellBody {
            command: shell["command"].as_str().expect("command").to_owned()
        }),
        shell
    );
    assert_eq!(json(CompactBody::default()), body("POST", "/compact", 0));
    assert_eq!(
        json(TitleBody {
            title: "Recorded title".to_owned()
        }),
        body("PATCH", "", 0)
    );

    let before = MessageId::parse("msg_15d824387001C5U5EIkeFSEWKC").expect("message id");
    let forks: Vec<Value> = find("POST", "/fork")
        .into_iter()
        .map(|exchange| exchange.body)
        .collect();
    assert_eq!(json(ForkBody { before: Some(before) }), forks[0]);
    assert_eq!(json(ForkBody::default()), forks[1]);

    let replies: Vec<Value> = find("POST", "/reply")
        .into_iter()
        .map(|exchange| exchange.body)
        .collect();
    let reply = |decision, message: Option<&str>| {
        json(PermissionReplyBody {
            decision,
            message: message.map(str::to_owned),
        })
    };
    assert!(replies.contains(&reply(Decision::Once, None)));
    assert!(replies.contains(&reply(Decision::Always, None)));
    assert!(replies.contains(&reply(Decision::Reject, Some("Do not change that file."))));

    let mut answer = FormReplyBody::default();
    answer
        .answer
        .insert("q0".to_owned(), FormValue::Text("blue".to_owned()));
    assert!(replies.contains(&json(&answer)));
}

#[test]
fn picker_bodies_name_the_agent_and_the_model() {
    let agent = json(AgentBody {
        agent: "plan".to_owned(),
    });
    assert_eq!(agent, json!({"agent": "plan"}));

    let model = json(ModelBody {
        model: ModelRef {
            variant: Some("high".to_owned()),
            ..big_pickle()
        },
    });
    assert_eq!(
        model,
        json!({"model": {"id": "big-pickle", "providerID": "opencode", "variant": "high"}})
    );

    let mut answers = FormReplyBody::default();
    answers
        .answer
        .insert("pick".to_owned(), FormValue::Many(vec!["a".to_owned(), "b".to_owned()]));
    answers.answer.insert("sure".to_owned(), FormValue::Bool(true));
    answers.answer.insert("count".to_owned(), FormValue::Number(3.0));
    assert_eq!(
        json(answers),
        json!({"answer": {"count": 3.0, "pick": ["a", "b"], "sure": true}})
    );
}

#[test]
fn local_files_become_file_uris() {
    let file = FileRef::local(Path::new("/home/me/my shots/a#1 ü.png")).expect("absolute");

    assert_eq!(file.uri, "file:///home/me/my%20shots/a%231%20%C3%BC.png");
    assert_eq!(file.name.as_deref(), Some("a#1 ü.png"));
    assert_eq!(FileRef::local(Path::new("shots/a.png")), None);
}

#[test]
fn objects_read_leniently() {
    let session: Session = serde_json::from_value(json!({
        "id": "ses_x1",
        "tokens": {"input": 12.6, "output": -3, "cache": {"read": 4}},
        "somethingNew": {"nested": true}
    }))
    .expect("a session with only an id");

    assert_eq!(session.title, None);
    assert_eq!(session.location.directory, "");
    assert_eq!(
        (session.tokens.input, session.tokens.output, session.tokens.reasoning),
        (13, 0, 0)
    );
    assert_eq!((session.tokens.cache.read, session.tokens.cache.write), (4, 0));

    let page: Envelope<Vec<Value>> = serde_json::from_value(json!({"data": [], "cursor": {}})).expect("page");
    assert_eq!(page.next_cursor(), None);

    let missing: Result<Envelope<Session>, _> = serde_json::from_value(json!({"data": {"title": "x"}}));
    assert!(missing.is_err(), "a session needs its id");
}
