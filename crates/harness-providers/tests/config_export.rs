//! Red tests: ProviderConfig last_model persistence + export template (spec/03).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use tempfile::tempdir;
use harness_providers::router::{ProviderConfig, ProviderEntry, ProviderKind};

#[test]
fn config_default_has_no_last_model() {
    let cfg = ProviderConfig::default_with_builtins();
    assert_eq!(cfg.ui.last_model, None);
}

#[test]
fn config_load_preserves_last_model() {
    let toml = r#"
[providers.anthropic]
kind = "anthropic"

[ui]
last_model = "anthropic/claude-sonnet-4-5"
"#;
    let cfg: ProviderConfig = toml::from_str(toml).unwrap();
    assert_eq!(cfg.ui.last_model, Some("anthropic/claude-sonnet-4-5".to_string()));
}

#[test]
fn config_save_writes_last_model() {
    let mut cfg = ProviderConfig::default_with_builtins();
    cfg.ui.last_model = Some("openai/gpt-4o".to_string());
    
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    cfg.save(&path).unwrap();
    
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("last_model = \"openai/gpt-4o\""));
}

#[test]
fn export_template_writes_providers_without_keys() {
    let mut cfg = ProviderConfig::default_with_builtins();
    // Add a custom provider
    cfg.add_custom("meu-nim", ProviderKind::Nim, "https://custom.example.com/v1");
    
    let dir = tempdir().unwrap();
    let path = dir.path().join("template.toml");
    cfg.export_template(&path).unwrap();
    
    let content = std::fs::read_to_string(&path).unwrap();
    // Should have providers section
    assert!(content.contains("[providers.anthropic]"));
    assert!(content.contains("[providers.openai]"));
    assert!(content.contains("[providers.google]"));
    assert!(content.contains("[providers.meu-nim]"));
    // Should have kind and base_url
    assert!(content.contains("kind = \"anthropic\""));
    assert!(content.contains("kind = \"openai\""));
    assert!(content.contains("kind = \"openai-compatible\""));
    assert!(content.contains("kind = \"nim\""));
    assert!(content.contains("base_url = \"https://custom.example.com/v1\""));
    // Should NOT have any keys (vault is separate)
    assert!(!content.contains("api_key"));
    assert!(!content.contains("secret"));
}

#[test]
fn export_template_excludes_last_model() {
    let mut cfg = ProviderConfig::default_with_builtins();
    cfg.ui.last_model = Some("anthropic/claude-sonnet-4-5".to_string());
    
    let dir = tempdir().unwrap();
    let path = dir.path().join("template.toml");
    cfg.export_template(&path).unwrap();
    
    let content = std::fs::read_to_string(&path).unwrap();
    // Template should not include UI section
    assert!(!content.contains("[ui]"));
    assert!(!content.contains("last_model"));
}

#[test]
fn provider_entry_serialization() {
    let entry = ProviderEntry {
        kind: Some(ProviderKind::Nim),
        base_url: Some("https://integrate.api.nvidia.com/v1".to_string()),
    };
    let toml_str = toml::to_string(&entry).unwrap();
    assert!(toml_str.contains("kind = \"nim\""));
    assert!(toml_str.contains("base_url = \"https://integrate.api.nvidia.com/v1\""));
}