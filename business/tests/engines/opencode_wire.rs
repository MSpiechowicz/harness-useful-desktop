//! The `opencode serve` JSON the chat engine reads and sends (`wire`), against the recorded run
//! (`fixtures/opencode/server`).

use crate::opencode_mapper_support;

use harness_business::model::ImageAttachment;
use harness_business::opencode_server::wire::{
    ModelRef, Outgoing, Settings, command_body, config_model, is_default_title, listening_addr, newest_message,
    primary_agents, prompt_body, provider_models, server_default_model, session_info, slash_command,
};
use opencode_mapper_support::{SESSION, recorded_body};
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn the_listening_line_gives_the_server_address() {
    let addr = listening_addr("opencode server listening on http://127.0.0.1:4096\n");
    assert_eq!(addr, Some("127.0.0.1:4096".parse().expect("address")));

    let ipv6 = listening_addr("opencode server listening on http://[::1]:39511");
    assert_eq!(ipv6, Some("[::1]:39511".parse().expect("address")));

    assert_eq!(listening_addr("Performing one time database migration..."), None);
    assert_eq!(
        listening_addr("opencode server listening on http://localhost:4096"),
        None
    );
}

#[test]
fn sessions_read_their_title_agent_and_model() {
    let created = session_info(&recorded_body("session-create.json")).expect("session");
    assert_eq!(created.id, SESSION);
    assert_eq!(created.directory, "/work/project");
    assert_eq!(created.title, None, "a default title is not a name");
    assert_eq!(created.model, None);

    let resumed = session_info(&recorded_body("session-get.json")).expect("session");
    assert_eq!(resumed.title.as_deref(), Some("Renamed chat"));
    assert_eq!(resumed.agent.as_deref(), Some("build"));
    assert_eq!(
        resumed.model,
        Some(ModelRef {
            provider: "mock".to_owned(),
            id: "m1".to_owned()
        })
    );
    assert_eq!(resumed.variant, None, "the `default` variant is no variant");

    assert!(is_default_title("Child session - 2026-10-19T19:36:01.225Z"));
    assert!(!is_default_title("New session - about the parser"));
}

#[test]
fn providers_give_display_fields_and_variants() {
    let models = provider_models(&recorded_body("config-providers.json"));

    let mock = models
        .iter()
        .find(|model| model.option.provider == "mock")
        .expect("mock model");
    assert_eq!(mock.option.id, "m1");
    assert_eq!(mock.option.name.as_deref(), Some("Mock One"));
    assert_eq!(mock.option.context_window, Some(100_000));
    assert_eq!(mock.option.max_tokens, Some(4096));
    assert_eq!(mock.option.input, vec!["text".to_owned()]);
    assert!(!mock.option.reasoning);
    assert!(mock.variants.is_empty());

    let longcat = models
        .iter()
        .find(|model| model.option.id == "longcat-2.5-preview-free")
        .expect("variant model");
    assert_eq!(longcat.variants, vec!["low", "medium", "high"]);
    assert_eq!(longcat.option.input, vec!["text".to_owned(), "image".to_owned()]);
    assert!(longcat.option.reasoning);

    let shown =
        serde_json::to_string(&models.iter().map(|model| &model.option).collect::<Vec<_>>()).expect("serializes");
    assert!(
        !shown.contains("baseURL") && !shown.contains("apiKey"),
        "no provider options"
    );
}

#[test]
fn only_visible_primary_agents_are_offered() {
    let agents = primary_agents(&recorded_body("agent-list.json"));
    let ids: Vec<&str> = agents.iter().map(|agent| agent.id.as_str()).collect();

    assert_eq!(ids, ["build", "plan"]);
    assert_eq!(
        agents[1].description.as_deref(),
        Some("Plan mode. Disallows all edit tools.")
    );
}

#[test]
fn the_prompt_body_carries_images_id_agent_model_and_variant_but_never_tools() {
    let model = ModelRef {
        provider: "mock".to_owned(),
        id: "m1".to_owned(),
    };
    let settings = Settings {
        agent: "plan",
        model: Some(&model),
        variant: Some("high"),
    };
    let mut look = Outgoing::new("Look", &[image()]);
    look.id = Some("msg_155aa0157001zEytp89Ql3qvh3".to_owned());

    assert_eq!(
        prompt_body(&look, &settings),
        json!({
            "parts": [
                { "type": "text", "text": "Look" },
                { "type": "file", "mime": "image/png", "filename": "image-1.png",
                  "url": "data:image/png;base64,iVBORw0KGgo=" },
            ],
            "agent": "plan",
            "model": { "providerID": "mock", "modelID": "m1" },
            "variant": "high",
            "messageID": "msg_155aa0157001zEytp89Ql3qvh3",
        })
    );

    let plain = Settings {
        agent: "build",
        model: None,
        variant: None,
    };
    assert_eq!(
        prompt_body(&Outgoing::new("Say hello", &[]), &plain),
        json!({ "parts": [{ "type": "text", "text": "Say hello" }], "agent": "build" })
    );
}

#[test]
fn a_command_body_names_the_model_as_provider_slash_model() {
    let model = ModelRef {
        provider: "mock".to_owned(),
        id: "m1".to_owned(),
    };
    let settings = Settings {
        agent: "build",
        model: Some(&model),
        variant: None,
    };
    let message = Outgoing::new("/review the parser", &[image()]);
    let (name, arguments) = slash_command(&message.text).expect("a command");

    assert_eq!(
        command_body(name, arguments, &message, &settings),
        json!({
            "command": "review",
            "arguments": "the parser",
            "agent": "build",
            "model": "mock/m1",
            "parts": [{ "type": "file", "mime": "image/png", "filename": "image-1.png",
                        "url": "data:image/png;base64,iVBORw0KGgo=" }],
        })
    );

    assert_eq!(slash_command("/init"), Some(("init", "")));
    assert_eq!(slash_command("/ x"), None);
    assert_eq!(slash_command("no command"), None);
}

#[test]
fn without_a_configured_model_the_first_allowed_providers_default_is_used() {
    let providers = recorded_body("config-providers.json");
    let model = |provider: &str, id: &str| {
        Some(ModelRef {
            provider: provider.to_owned(),
            id: id.to_owned(),
        })
    };

    let configured = json!({ "model": "mock/m1" });
    assert_eq!(server_default_model(&configured, &providers), model("mock", "m1"));

    let first = providers["providers"][0]["id"].as_str().expect("a provider");
    let first_default = providers["default"][first].as_str().expect("its default");
    assert_eq!(
        server_default_model(&json!({}), &providers),
        model(first, first_default)
    );

    let only_mock = json!({ "provider": { "mock": {} } });
    assert_eq!(server_default_model(&only_mock, &providers), model("mock", "m1"));
    assert_eq!(server_default_model(&json!({}), &json!({ "providers": [] })), None);
}

#[test]
fn the_configured_default_model_and_the_newest_message_are_read() {
    let model = config_model(&json!({ "model": "mock/m1", "small_model": "mock/m0" }));
    assert_eq!(
        model,
        Some(ModelRef {
            provider: "mock".to_owned(),
            id: "m1".to_owned()
        })
    );
    assert_eq!(config_model(&json!({ "share": "disabled" })), None);
    assert_eq!(config_model(&json!({ "model": "no-slash" })), None);

    let page = recorded_body("messages.json");
    let (role, id) = newest_message(&page).expect("newest message");
    let last = &page.as_array().expect("a list").last().expect("a message")["info"];
    assert_eq!(
        (role.as_str(), id.as_str()),
        (last["role"].as_str().unwrap(), last["id"].as_str().unwrap())
    );
}

fn image() -> ImageAttachment {
    ImageAttachment {
        data: "iVBORw0KGgo=".to_owned(),
        mime_type: "image/png".to_owned(),
    }
}
