//! Vault local de chaves de API, criptografado com age (spec/03, D11).
//!
//! Conteúdo: TOML `provider_id = "api_key"` criptografado com passphrase.
//! A passphrase vem do env `HARNIZATOR_VAULT_KEY` (OS keyring: wave futura).
//!
//! O mapa decriptado fica em cache na memória (Mutex) e é recarregado
//! apenas quando o arquivo muda no disco (mtime). Isso evita rodar o
//! KDF scrypt a cada leitura — era o que travava a UI.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use secrecy::{ExposeSecret, SecretString};

pub const KEY_ENV_VAR: &str = "HARNIZATOR_VAULT_KEY";

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("vault key material missing: set {KEY_ENV_VAR}")]
    MissingKeyMaterial,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to decrypt vault (wrong key or corrupted file)")]
    Decrypt,
    #[error("failed to encrypt vault")]
    Encrypt,
    #[error("failed to parse vault contents")]
    Parse,
}

/// Vault de chaves (`~/.config/harnizator/vault.age`).
#[derive(Clone)]
pub struct Vault {
    path: PathBuf,
    key: Arc<SecretString>,
    /// (mtime do arquivo, conteúdo decriptado). `None` = ainda não carregado.
    cache: Arc<Mutex<Option<(Option<SystemTime>, BTreeMap<String, String>)>>>,
}

impl Vault {
    /// Abre o vault lendo a passphrase do env `HARNIZATOR_VAULT_KEY`.
    pub fn open(path: &Path) -> Result<Self, VaultError> {
        let key = std::env::var(KEY_ENV_VAR).map_err(|_| VaultError::MissingKeyMaterial)?;
        Self::open_with_key(path, SecretString::from(key))
    }

    /// Abre o vault com uma passphrase explícita (testes, keyring futuro).
    pub fn open_with_key(path: &Path, key: SecretString) -> Result<Self, VaultError> {
        let vault = Self {
            path: path.to_path_buf(),
            key: Arc::new(key),
            cache: Arc::new(Mutex::new(None)),
        };
        // valida a chave contra o conteúdo existente (ou arquivo ausente)
        vault.load()?;
        Ok(vault)
    }

    fn file_mtime(&self) -> Option<SystemTime> {
        std::fs::metadata(&self.path)
            .and_then(|m| m.modified())
            .ok()
    }

    /// Carrega o mapa decriptado, usando cache se o arquivo não mudou.
    fn load(&self) -> Result<BTreeMap<String, String>, VaultError> {
        let mtime = self.file_mtime();
        {
            let cache = match self.cache.lock() {
                Ok(c) => c,
                Err(p) => p.into_inner(),
            };
            if let Some((cached_mtime, map)) = &*cache {
                if *cached_mtime == mtime {
                    return Ok(map.clone());
                }
            }
        }
        let map = self.load_from_disk()?;
        let mut cache = match self.cache.lock() {
            Ok(c) => c,
            Err(p) => p.into_inner(),
        };
        *cache = Some((mtime, map.clone()));
        Ok(map)
    }

    fn load_from_disk(&self) -> Result<BTreeMap<String, String>, VaultError> {
        let mut data = Vec::new();
        match std::fs::File::open(&self.path) {
            Ok(mut f) => {
                f.read_to_end(&mut data)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
            Err(e) => return Err(VaultError::Io(e)),
        }
        if data.is_empty() {
            return Ok(BTreeMap::new());
        }
        let decryptor = age::Decryptor::new(&data[..]).map_err(|_| VaultError::Decrypt)?;
        if !decryptor.is_scrypt() {
            return Err(VaultError::Decrypt);
        }
        let identity = age::scrypt::Identity::new(age::secrecy::SecretString::from(
            self.key.expose_secret().to_string(),
        ));
        let mut plain = Vec::new();
        let mut reader = decryptor
            .decrypt(std::iter::once(&identity as &dyn age::Identity))
            .map_err(|_| VaultError::Decrypt)?;
        reader
            .read_to_end(&mut plain)
            .map_err(|_| VaultError::Decrypt)?;
        toml::from_str(&String::from_utf8_lossy(&plain)).map_err(|_| VaultError::Parse)
    }

    fn save(&self, map: &BTreeMap<String, String>) -> Result<(), VaultError> {
        let plain = toml::to_string(map).map_err(|_| VaultError::Encrypt)?;
        let encryptor = age::Encryptor::with_user_passphrase(age::secrecy::SecretString::from(
            self.key.expose_secret().to_string(),
        ));
        let mut encrypted = Vec::new();
        {
            let mut writer = encryptor
                .wrap_output(&mut encrypted)
                .map_err(|_| VaultError::Encrypt)?;
            writer
                .write_all(plain.as_bytes())
                .map_err(|_| VaultError::Encrypt)?;
            writer.finish().map_err(|_| VaultError::Encrypt)?;
        }
        let tmp = self.path.with_extension("age.tmp");
        {
            let mut f = std::fs::File::create(&tmp)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
            }
            f.write_all(&encrypted)?;
        }
        std::fs::rename(&tmp, &self.path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600))?;
        }
        // atualiza o cache com o novo conteúdo
        let mtime = self.file_mtime();
        let mut cache = match self.cache.lock() {
            Ok(c) => c,
            Err(p) => p.into_inner(),
        };
        *cache = Some((mtime, map.clone()));
        Ok(())
    }

    /// Define/atualiza a chave de um provider (persiste imediatamente).
    pub fn set(&mut self, provider_id: &str, key: SecretString) -> Result<(), VaultError> {
        let mut map = self.load()?;
        map.insert(provider_id.to_string(), key.expose_secret().to_string());
        self.save(&map)
    }

    /// Lê a chave de um provider (redacted em Debug via secrecy).
    pub fn get(&self, provider_id: &str) -> Result<Option<SecretString>, VaultError> {
        Ok(self
            .load()?
            .get(provider_id)
            .map(|k| SecretString::from(k.clone())))
    }

    /// Remove a chave de um provider.
    pub fn delete(&mut self, provider_id: &str) -> Result<(), VaultError> {
        let mut map = self.load()?;
        map.remove(provider_id);
        self.save(&map)
    }

    /// Lista os provider_ids configurados.
    pub fn list_ids(&self) -> Vec<String> {
        self.load()
            .map(|m| m.into_keys().collect())
            .unwrap_or_default()
    }
}
