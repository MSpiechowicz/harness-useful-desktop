//! Share bundles: `hud1:` strings and JSON files round-trip, damaged or oversized input is rejected
//! as a whole, and one bad item only marks itself invalid.

use crate::sync_support::servers;

use std::io::Write;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use flate2::Compression;
use flate2::write::ZlibEncoder;
use harness_business::BusinessError;
use harness_business::model::{HarnessId, IpcErrorCode, SyncInvalidReason, SyncUnsupported};
use harness_business::sync::codec::{
    DecodeError, DecodedBundle, decode_file, decode_input, encode_code, to_json_pretty,
};
use harness_business::sync::schema::{
    Bundle, HarnessSection, MAX_CODE_BYTES, MAX_JSON_BYTES, MAX_PASTE_BYTES, Marketplace, Package, Parsed, Plugin,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use servers::{http, stdio};

fn sample() -> Bundle {
    let mut bundle = Bundle::new(Some("0.15.0".to_owned()), Some("2026-10-15".to_owned()));
    let mut github = stdio("github", "npx", &["-y", "@modelcontextprotocol/server-github"]);
    github.env_keys = vec!["GITHUB_TOKEN".to_owned()];
    let mut docs = http("docs", "https://mcp.example.com/mcp");
    docs.header_keys = vec!["Authorization".to_owned()];

    bundle.harnesses.insert(
        HarnessId::Omp,
        HarnessSection {
            marketplaces: vec![Marketplace {
                name: "official".to_owned(),
                source: "owner/repo".to_owned(),
            }],
            plugins: vec![Plugin {
                id: "review@official".to_owned(),
                marketplace: Some("official".to_owned()),
                version: Some("1.2.0".to_owned()),
            }],
            packages: Vec::new(),
            mcp_servers: vec![github, docs],
        },
    );
    bundle.harnesses.insert(
        HarnessId::Pi,
        HarnessSection {
            packages: vec![Package {
                source: "npm:@scope/pkg@1.2.0".to_owned(),
                version: None,
            }],
            ..HarnessSection::default()
        },
    );

    bundle
}

/// Rebuilds a bundle from the valid items of a decoded one.
fn valid_items(decoded: &DecodedBundle) -> Bundle {
    let mut bundle = Bundle::new(decoded.app.clone(), decoded.created_at.clone());

    fn ok<T: Clone + std::fmt::Debug>(items: &[Parsed<T>]) -> Vec<T> {
        items.iter().map(|parsed| parsed.item.clone().unwrap()).collect()
    }

    for section in &decoded.sections {
        bundle.harnesses.insert(
            section.harness,
            HarnessSection {
                marketplaces: ok(&section.marketplaces),
                plugins: ok(&section.plugins),
                packages: ok(&section.packages),
                mcp_servers: ok(&section.mcp_servers),
            },
        );
    }

    bundle
}

fn code_of_bytes(json: &[u8]) -> String {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(json).unwrap();
    format!("hud1:{}", URL_SAFE_NO_PAD.encode(encoder.finish().unwrap()))
}

fn bundle_json(harnesses: Value) -> String {
    json!({ "format": "harness-useful-desktop/setup", "version": 1, "harnesses": harnesses }).to_string()
}

fn decode_err(text: &str) -> DecodeError {
    decode_input(text).expect_err("input must be rejected")
}

#[test]
fn share_string_and_file_round_trip() {
    let bundle = sample();

    let code = encode_code(&bundle).unwrap();
    assert!(code.starts_with("hud1:"));
    let decoded = decode_input(&code).unwrap();
    assert_eq!(decoded.invalid_count(), 0);
    assert_eq!(decoded.unsupported, Vec::new());
    assert_eq!(valid_items(&decoded), bundle);

    let file = to_json_pretty(&bundle).unwrap();
    assert_eq!(valid_items(&decode_file(file.as_bytes()).unwrap()), bundle);
    assert_eq!(valid_items(&decode_input(&file).unwrap()), bundle);
}

#[test]
fn whitespace_around_and_inside_the_string_is_ignored() {
    let code = encode_code(&sample()).unwrap();
    let chunks: Vec<String> = code
        .as_bytes()
        .chunks(20)
        .map(|chunk| String::from_utf8(chunk.to_vec()).unwrap())
        .collect();
    let wrapped = format!("  \n{}\r\n\t ", chunks.join("\n   "));

    assert_eq!(valid_items(&decode_input(&wrapped).unwrap()), sample());
}

#[test]
fn damaged_strings_are_rejected() {
    let code = encode_code(&sample()).unwrap();

    // Without its last 3 bytes the stream has no complete Adler-32 trailer.
    let payload = URL_SAFE_NO_PAD.decode(&code["hud1:".len()..]).unwrap();
    let truncated = format!("hud1:{}", URL_SAFE_NO_PAD.encode(&payload[..payload.len() - 3]));
    assert_eq!(decode_err(&truncated), DecodeError::Decompress);
    for cut in 1..8 {
        assert!(decode_input(&code[..code.len() - cut]).is_err(), "cut {cut}");
    }
    assert_eq!(decode_err("hud1:!!!!"), DecodeError::Base64);
    assert_eq!(decode_err("hud1:"), DecodeError::Decompress);
    assert_eq!(decode_err(&code.replacen("hud1:", "HUD1:", 1)), DecodeError::Prefix);
    assert_eq!(decode_err(&code["hud1:".len()..]), DecodeError::Prefix);
    assert_eq!(decode_err(" \n\t"), DecodeError::Empty);
    let trailing = [payload.as_slice(), b"extra"].concat();
    assert_eq!(
        decode_err(&format!("hud1:{}", URL_SAFE_NO_PAD.encode(trailing))),
        DecodeError::Decompress
    );
}

#[test]
fn size_limits_hold_at_their_boundaries() {
    // A zlib bomb: 1 MiB of zeros compresses to about 1 KiB.
    assert_eq!(
        decode_err(&code_of_bytes(&vec![0; 1024 * 1024])),
        DecodeError::DecompressedTooLarge
    );

    let at_limit = format!("hud1:{}", "A".repeat(MAX_PASTE_BYTES - "hud1:".len()));
    assert_ne!(decode_err(&at_limit), DecodeError::TooLarge);
    assert_eq!(decode_err(&format!("{at_limit}A")), DecodeError::TooLarge);
    assert_eq!(decode_err(&"x".repeat(1024 * 1024)), DecodeError::TooLarge);

    assert_eq!(
        decode_file(&vec![b' '; MAX_JSON_BYTES + 1]).unwrap_err(),
        DecodeError::TooLarge
    );
    let padded = format!("{}{}", bundle_json(json!({})), " ".repeat(MAX_PASTE_BYTES));
    assert!(padded.len() <= MAX_JSON_BYTES);
    assert!(decode_file(padded.as_bytes()).is_ok());
}

#[test]
fn deep_nesting_is_an_error_not_a_crash() {
    let nested = format!(
        r#"{{"format":"harness-useful-desktop/setup","version":1,"x":{}}}"#,
        "[".repeat(10_000)
    );

    assert_eq!(decode_err(&nested), DecodeError::Json);
    assert_eq!(decode_err(&code_of_bytes(nested.as_bytes())), DecodeError::Json);
}

#[test]
fn format_and_version_are_checked() {
    let with = |format: Value, version: Value| json!({ "format": format, "version": version }).to_string();

    assert_eq!(
        decode_err(&with(json!("harness-useful-desktop/setup"), json!(2))),
        DecodeError::NewerVersion
    );
    assert_eq!(decode_err(&with(json!("other/setup"), json!(1))), DecodeError::Format);
    assert_eq!(
        decode_err(&with(json!("Harness-Useful-Desktop/setup"), json!(1))),
        DecodeError::Format
    );
    assert_eq!(
        decode_err(&with(json!("harness-useful-desktop/setup"), json!(0))),
        DecodeError::Version
    );
    assert_eq!(
        decode_err(&with(json!("harness-useful-desktop/setup"), json!("1"))),
        DecodeError::Version
    );
    assert_eq!(decode_err(&code_of_bytes(b"[]")), DecodeError::NotAnObject);
    assert_eq!(decode_err(&bundle_json(json!({ "omp": [] }))), DecodeError::NotAnObject);

    let newer = BusinessError::from(DecodeError::NewerVersion);
    assert_eq!(newer.code(), IpcErrorCode::Unsupported);
    assert_eq!(
        BusinessError::from(DecodeError::Base64).code(),
        IpcErrorCode::InvalidInput
    );
}

#[test]
fn unknown_harnesses_kinds_and_fields_are_counted_and_ignored() {
    let text = json!({
        "format": "harness-useful-desktop/setup", "version": 1, "future": { "x": 1 },
        "harnesses": {
            "omp": { "plugins": [{ "id": "review@official", "extra": true }], "themes": [{}, {}], "note": "x" },
            "acme": { "skills": [1, 2], "agents": [3] },
            "bad key!": { "y": [1] },
            "codex": "not an object",
        },
    })
    .to_string();

    let decoded = decode_input(&text).unwrap();

    assert_eq!(decoded.sections.len(), 1);
    assert_eq!(
        decoded.sections[0].plugins[0].item.as_ref().unwrap().id,
        "review@official"
    );
    assert_eq!(
        decoded.unsupported,
        vec![
            SyncUnsupported {
                harness: Some(HarnessId::Omp),
                name: "themes".to_owned(),
                item_count: 2
            },
            SyncUnsupported {
                harness: None,
                name: "acme".to_owned(),
                item_count: 3
            },
            SyncUnsupported {
                harness: None,
                name: "unknown".to_owned(),
                item_count: 1
            },
            SyncUnsupported {
                harness: None,
                name: "codex".to_owned(),
                item_count: 0
            },
        ]
    );
}

#[test]
fn one_invalid_item_does_not_fail_the_others() {
    let text = bundle_json(json!({ "pi": { "mcpServers": [
        { "name": "fine", "transport": "http", "url": "https://mcp.example.com" },
        { "name": 5, "transport": "stdio", "command": "npx" },
        { "name": "shell", "transport": "stdio", "command": "bash" },
        "not an object",
        { "name": "ghp_abcdefghijklmnopqrstuvwxyz0123456789", "transport": "stdio", "command": "npx" },
    ] } }));

    let decoded = decode_input(&text).unwrap();
    let servers = &decoded.sections[0].mcp_servers;
    let outcome: Vec<_> = servers
        .iter()
        .map(|parsed| (parsed.label.as_deref(), parsed.item.as_ref().err().copied()))
        .collect();

    assert_eq!(
        outcome,
        vec![
            (Some("fine"), None),
            (None, Some(SyncInvalidReason::Malformed)),
            (Some("shell"), Some(SyncInvalidReason::ShellCommand)),
            (None, Some(SyncInvalidReason::Malformed)),
            (None, Some(SyncInvalidReason::ContainsSecret)),
        ]
    );
    assert_eq!(decoded.invalid_count(), 4);
}

#[test]
fn item_and_section_counts_are_capped() {
    let packages = |count: usize| -> Value {
        (0..count)
            .map(|index| json!({ "source": format!("npm:p{index}") }))
            .collect()
    };

    assert!(decode_file(bundle_json(json!({ "pi": { "packages": packages(200) } })).as_bytes()).is_ok());
    assert_eq!(
        decode_file(bundle_json(json!({ "pi": { "packages": packages(201) } })).as_bytes()).unwrap_err(),
        DecodeError::TooManyItems
    );

    let total = json!({ "pi": { "packages": packages(200), "mcpServers": packages(200) }, "omp": { "plugins": packages(100) } });
    assert!(decode_file(bundle_json(total).as_bytes()).is_ok());
    let over = json!({ "pi": { "packages": packages(200), "mcpServers": packages(200) }, "omp": { "plugins": packages(101) } });
    assert_eq!(
        decode_file(bundle_json(over).as_bytes()).unwrap_err(),
        DecodeError::TooManyItems
    );

    let sections = |count: usize| -> Value {
        (0..count)
            .map(|index| (format!("h{index}"), json!({})))
            .collect::<serde_json::Map<_, _>>()
            .into()
    };
    assert!(decode_input(&bundle_json(sections(16))).is_ok());
    assert_eq!(decode_err(&bundle_json(sections(17))), DecodeError::TooManySections);
}

#[test]
fn a_setup_too_large_for_a_string_still_fits_a_file() {
    // Pseudo-random lowercase sources (no digits: not a secret-looking token) compress poorly, so the string outgrows its limit long before the JSON does.
    let mut state: u32 = 7;
    let mut random_source = || -> String {
        let chars: String = (0..480)
            .map(|_| {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                char::from(b"abcdefghijklmnopqrstuvwxyz"[(state >> 16) as usize % 26])
            })
            .collect();
        format!("npm:{chars}")
    };

    let mut bundle = Bundle::new(None, None);
    let packages = (0..200)
        .map(|_| Package {
            source: random_source(),
            version: None,
        })
        .collect();
    bundle.harnesses.insert(
        HarnessId::Pi,
        HarnessSection {
            packages,
            ..HarnessSection::default()
        },
    );

    let error = encode_code(&bundle).unwrap_err();
    assert!(matches!(error, BusinessError::InvalidInput(_)), "{error}");
    let file = to_json_pretty(&bundle).unwrap();
    assert!(file.len() > MAX_CODE_BYTES);
    assert_eq!(decode_file(file.as_bytes()).unwrap().invalid_count(), 0);
}
