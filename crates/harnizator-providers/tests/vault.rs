//! Red tests: vault de chaves age (spec/03, D11).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use harnizator_providers::vault::Vault;
use secrecy::{ExposeSecret, SecretString};

fn key() -> SecretString {
    SecretString::from("test-passphrase".to_string())
}

#[test]
fn roundtrip_set_get() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.age");
    let mut vault = Vault::open_with_key(&path, key()).unwrap();
    vault
        .set("anthropic", SecretString::from("sk-ant-123".to_string()))
        .unwrap();
    drop(vault);

    let vault = Vault::open_with_key(&path, key()).unwrap();
    assert_eq!(
        vault.get("anthropic").unwrap().unwrap().expose_secret(),
        "sk-ant-123"
    );
}

#[test]
fn wrong_key_fails_to_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.age");
    let mut vault = Vault::open_with_key(&path, key()).unwrap();
    vault.set("a", SecretString::from("1".to_string())).unwrap();
    drop(vault);

    let bad = SecretString::from("wrong".to_string());
    assert!(Vault::open_with_key(&path, bad).is_err());
}

#[test]
fn list_and_delete() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.age");
    let mut vault = Vault::open_with_key(&path, key()).unwrap();
    vault
        .set("openai", SecretString::from("k1".to_string()))
        .unwrap();
    vault
        .set("google", SecretString::from("k2".to_string()))
        .unwrap();
    let mut ids = vault.list_ids();
    ids.sort();
    assert_eq!(ids, vec!["google", "openai"]);
    vault.delete("openai").unwrap();
    assert!(vault.get("openai").unwrap().is_none());
}

#[cfg(unix)]
#[test]
fn vault_file_has_0600_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.age");
    let mut vault = Vault::open_with_key(&path, key()).unwrap();
    vault.set("a", SecretString::from("1".to_string())).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn get_is_redacted_in_debug() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.age");
    let mut vault = Vault::open_with_key(&path, key()).unwrap();
    vault
        .set("a", SecretString::from("super-secret".to_string()))
        .unwrap();
    let dbg = format!("{:?}", vault.get("a").unwrap().unwrap());
    assert!(!dbg.contains("super-secret"));
}

#[test]
fn cache_sees_external_writes_via_mtime() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.keep().join("vault.age");
    let key = secrecy::SecretString::from("k".to_string());
    let mut vault = Vault::open_with_key(&path, key.clone()).unwrap();
    vault
        .set("a", secrecy::SecretString::from("1".to_string()))
        .unwrap();

    // outra instância compartilhando o arquivo escreve por fora
    let mut other = Vault::open_with_key(&path, key).unwrap();
    other
        .set("b", secrecy::SecretString::from("2".to_string()))
        .unwrap();

    // força mtime distinto (algumas FS têm granularidade de 1s)
    std::thread::sleep(std::time::Duration::from_millis(10));

    assert_eq!(
        vault
            .get("b")
            .unwrap()
            .map(|s| secrecy::ExposeSecret::expose_secret(&s).to_string()),
        Some("2".to_string()),
        "cache deve invalidar quando o arquivo muda"
    );
}
