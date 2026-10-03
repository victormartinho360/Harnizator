//! Red tests: config com defaults embutidos + save/load (spec/03, Wave 6).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use harness_core::ModelAlias;
use harness_providers::router::{ProviderConfig, ProviderKind, ProviderRouter, RouterError};
use harness_providers::vault::Vault;
use secrecy::SecretString;

fn vault_with(ids: &[&str]) -> Vault {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.keep().join("vault.age");
    let mut v = Vault::open_with_key(&path, SecretString::from("k".to_string())).unwrap();
    for id in ids {
        v.set(id, SecretString::from(format!("sk-{id}"))).unwrap();
    }
    v
}

#[test]
fn defaults_include_anthropic_openai_google() {
    let cfg = ProviderConfig::default_with_builtins();
    assert!(cfg.providers.contains_key("anthropic"));
    assert!(cfg.providers.contains_key("openai"));
    assert!(cfg.providers.contains_key("google"));
}

#[test]
fn google_defaults_to_openai_compatible_endpoint() {
    let cfg = ProviderConfig::default_with_builtins();
    let google = cfg.providers.get("google").unwrap();
    assert_eq!(
        google.kind_named("google").unwrap(),
        ProviderKind::OpenAiCompatible
    );
    assert_eq!(
        google.base_url.as_deref(),
        Some("https://generativelanguage.googleapis.com/v1beta/openai")
    );
}

#[test]
fn google_alias_resolves_with_default_base_url() {
    let cfg = ProviderConfig::default_with_builtins();
    let v = vault_with(&["google"]);
    let router = ProviderRouter::new(&cfg, &v).unwrap();
    let alias = ModelAlias::parse("google/gemini-2.5-pro").unwrap();
    let resolved = router.resolve(&alias).unwrap();
    assert_eq!(resolved.provider.id(), "google");
    assert_eq!(resolved.model, "gemini-2.5-pro");
    assert_eq!(
        resolved.base_url,
        "https://generativelanguage.googleapis.com/v1beta/openai"
    );
}

#[test]
fn save_and_load_roundtrip_custom_provider() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let mut cfg = ProviderConfig::default_with_builtins();
    cfg.add_custom(
        "meu-proxy",
        ProviderKind::OpenAiCompatible,
        "http://localhost:8080/v1",
    );
    cfg.save(&path).unwrap();

    let loaded = ProviderConfig::load_or_default(&path);
    let entry = loaded.providers.get("meu-proxy").unwrap();
    assert_eq!(
        entry.kind_named("meu-proxy").unwrap(),
        ProviderKind::OpenAiCompatible
    );
    assert_eq!(entry.base_url.as_deref(), Some("http://localhost:8080/v1"));
}

#[test]
fn remove_custom_provider_keeps_builtins() {
    let mut cfg = ProviderConfig::default_with_builtins();
    cfg.add_custom("x", ProviderKind::OpenAi, "http://x");
    assert!(cfg.remove("x"));
    assert!(cfg.providers.contains_key("anthropic"));
    assert!(!cfg.providers.contains_key("x"));
}

#[test]
fn unknown_kind_in_file_errors_on_resolve() {
    let cfg: ProviderConfig = toml::from_str("[providers.xyz]\nbase_url = \"http://l\"").unwrap();
    let v = vault_with(&["xyz"]);
    let router = ProviderRouter::new(&cfg, &v).unwrap();
    let alias = ModelAlias::parse("xyz/model").unwrap();
    assert!(matches!(
        router.resolve(&alias),
        Err(RouterError::UnknownKind(_))
    ));
}
