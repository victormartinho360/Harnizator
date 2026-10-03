//! Red tests: ProviderRouter — aliases, defaults, overrides (spec/03, D17).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use harness_core::ModelAlias;
use harness_providers::router::{ProviderConfig, ProviderKind, ProviderRouter, RouterError};
use harness_providers::vault::Vault;
use secrecy::SecretString;

fn config() -> ProviderConfig {
    toml::from_str(
        r#"
[providers.anthropic]

[providers.meu-proxy]
kind = "openai-compatible"
base_url = "http://localhost:8080/v1"
"#,
    )
    .unwrap()
}

fn vault_with_anthropic() -> Vault {
    let dir = tempfile::tempdir().unwrap();
    // vazamento intencional do TempDir: o arquivo fica no tmp do SO nos testes
    let path = dir.keep().join("vault.age");
    let mut v = Vault::open_with_key(&path, SecretString::from("k".to_string())).unwrap();
    v.set("anthropic", SecretString::from("sk-ant".to_string()))
        .unwrap();
    v
}

#[test]
fn resolves_alias_for_known_provider() {
    let router = ProviderRouter::new(&config(), &vault_with_anthropic()).unwrap();
    let alias = ModelAlias::parse("anthropic/claude-sonnet-4-5").unwrap();
    let resolved = router.resolve(&alias).unwrap();
    assert_eq!(resolved.provider.id(), "anthropic");
    assert_eq!(resolved.model, "claude-sonnet-4-5");
}

#[test]
fn unknown_provider_errors() {
    let router = ProviderRouter::new(&config(), &vault_with_anthropic()).unwrap();
    let alias = ModelAlias::parse("openai/gpt-x").unwrap();
    assert!(matches!(
        router.resolve(&alias),
        Err(RouterError::UnknownProvider(_))
    ));
}

#[test]
fn missing_key_in_vault_errors() {
    let router = ProviderRouter::new(&config(), &vault_with_anthropic()).unwrap();
    let alias = ModelAlias::parse("meu-proxy/llama").unwrap();
    assert!(matches!(
        router.resolve(&alias),
        Err(RouterError::MissingKey(_))
    ));
}

#[test]
fn base_url_override_is_honored() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.keep().join("vault.age");
    let mut v = Vault::open_with_key(&path, SecretString::from("k".to_string())).unwrap();
    v.set("meu-proxy", SecretString::from("sk-p".to_string()))
        .unwrap();
    let router = ProviderRouter::new(&config(), &v).unwrap();
    let alias = ModelAlias::parse("meu-proxy/llama").unwrap();
    let resolved = router.resolve(&alias).unwrap();
    assert_eq!(resolved.provider.id(), "meu-proxy");
    assert_eq!(resolved.base_url, "http://localhost:8080/v1");
}

#[test]
fn anthropic_default_base_url() {
    let router = ProviderRouter::new(&config(), &vault_with_anthropic()).unwrap();
    let alias = ModelAlias::parse("anthropic/claude-sonnet-4-5").unwrap();
    let resolved = router.resolve(&alias).unwrap();
    assert_eq!(resolved.base_url, "https://api.anthropic.com/v1");
}

#[test]
fn config_kind_defaults_from_name() {
    // providers.anthropic sem `kind` ⇒ kind infere do nome
    let cfg: ProviderConfig = toml::from_str("[providers.anthropic]").unwrap();
    assert_eq!(
        cfg.providers["anthropic"].kind_named("anthropic").unwrap(),
        ProviderKind::Anthropic
    );
}
