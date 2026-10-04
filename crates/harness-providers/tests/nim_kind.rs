//! Red tests: NIM provider kind (spec/03).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use harness_providers::router::{ProviderConfig, ProviderKind};

#[test]
fn nim_kind_parse() {
    assert_eq!(ProviderKind::parse("nim"), Some(ProviderKind::Nim));
}

#[test]
fn nim_kind_serde() {
    let kind = ProviderKind::Nim;
    let serialized = serde_json::to_string(&kind).unwrap();
    assert_eq!(serialized, "\"nim\"");
    let deserialized: ProviderKind = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, ProviderKind::Nim);
}

#[test]
fn nim_kind_named() {
    // providers.nim sem kind explícito ⇒ infere do nome
    let cfg: ProviderConfig = toml::from_str("[providers.nim]").unwrap();
    assert_eq!(
        cfg.providers["nim"].kind_named("nim").unwrap(),
        ProviderKind::Nim
    );
}

#[test]
fn nim_default_base_url() {
    use harness_providers::router::ProviderEntry;

    let entry = ProviderEntry {
        kind: Some(ProviderKind::Nim),
        base_url: None,
    };
    let base = entry.base_url(ProviderKind::Nim);
    // NIM usa endpoint OpenAI-compatible da NVIDIA
    assert_eq!(base, "https://integrate.api.nvidia.com/v1");
}

#[test]
fn nim_custom_base_url_override() {
    use harness_providers::router::ProviderEntry;

    let entry = ProviderEntry {
        kind: Some(ProviderKind::Nim),
        base_url: Some("https://custom-nim.example.com/v1".into()),
    };
    let base = entry.base_url(ProviderKind::Nim);
    assert_eq!(base, "https://custom-nim.example.com/v1");
}