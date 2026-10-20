//! The team database URL as saved to Harness Memory's `config.toml`: MongoDB credentials are
//! percent-encoded once, other kinds are kept as typed.

use harness_business::ecosystem::{MemoryRemoteChange, ScriptsSupport, memory_remote_set};
use harness_business::model::MemoryRemoteKind;
use memory_remote_support::{Home, postgres_change};
use pretty_assertions::assert_eq;

mod memory_remote_support;

/// Saves `url` as a `kind` team database and returns the `database_url` the file holds.
fn save(home: &Home, kind: MemoryRemoteKind, url: &str) -> String {
    let change = MemoryRemoteChange {
        kind: Some(kind),
        url: Some(url.to_owned()),
        ..postgres_change()
    };
    memory_remote_set(&home.env, change, ScriptsSupport::Unsupported).unwrap();

    let file: toml::Table = toml::from_str(&std::fs::read_to_string(home.config()).unwrap()).unwrap();
    file["database_url"].as_str().unwrap().to_owned()
}

#[test]
fn mongodb_credentials_are_percent_encoded_once() {
    let home = Home::new();
    let mongodb = |url: &str| save(&home, MemoryRemoteKind::Mongodb, url);

    assert_eq!(
        mongodb("mongodb://me@x.com:p@ss:w/rd?#%zz@h.example.com:27017/db?tls=true"),
        "mongodb://me%40x.com:p%40ss%3Aw%2Frd%3F%23%25zz@h.example.com:27017/db?tls=true"
    );
    assert_eq!(
        mongodb("mongodb+srv://u:p%40ss@c.example.net/"),
        "mongodb+srv://u:p%40ss@c.example.net/"
    );
    assert_eq!(mongodb("mongodb://h.example.com/db"), "mongodb://h.example.com/db");
    assert_eq!(mongodb("mongodb://u@h/db"), "mongodb://u@h/db");

    let postgres = "postgres://u:p@ss@h/db";
    assert_eq!(save(&home, MemoryRemoteKind::Postgres, postgres), postgres);
}
