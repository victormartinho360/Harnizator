//! ProviderAdminService: admin de providers via config + vault (spec/03, 06-w6).

use std::path::PathBuf;
use std::sync::Mutex;

use harness_core::ModelAlias;
use harness_core::provider_admin::{ProviderAdmin, ProviderConfigView};

use crate::router::{ProviderConfig, ProviderKind, ProviderRouter};
use crate::vault::Vault;

/// Administra providers persistindo no config TOML e no vault age.
pub struct ProviderAdminService {
    config_path: PathBuf,
    config: Mutex<ProviderConfig>,
    vault: Mutex<Option<Vault>>,
}

impl ProviderAdminService {
    pub fn new(config_path: PathBuf, vault: Option<Vault>) -> Self {
        let config = ProviderConfig::load_or_default(&config_path);
        Self {
            config_path,
            config: Mutex::new(config),
            vault: Mutex::new(vault),
        }
    }

    fn save(&self) -> Result<(), String> {
        let cfg = self.config.lock().map_err(|e| e.to_string())?;
        cfg.save(&self.config_path).map_err(|e| e.to_string())
    }
}

#[async_trait::async_trait]
impl ProviderAdmin for ProviderAdminService {
    fn views(&self) -> Vec<ProviderConfigView> {
        let cfg = match self.config.lock() {
            Ok(c) => c,
            Err(p) => p.into_inner(),
        };
        let vault = match self.vault.lock() {
            Ok(v) => v,
            Err(p) => p.into_inner(),
        };
        cfg.providers
            .iter()
            .map(|(id, e)| {
                let kind = e
                    .kind_named(id)
                    .map(|k| match k {
                        ProviderKind::Anthropic => "anthropic",
                        ProviderKind::OpenAi => "openai",
                        ProviderKind::OpenAiCompatible => "openai-compatible",
                        ProviderKind::Nim => "nim",
                    })
                    .unwrap_or("unknown")
                    .to_string();
                let base = e.base_url.clone().unwrap_or_else(|| {
                    e.base_url(e.kind_named(id).unwrap_or(ProviderKind::OpenAi))
                });
                let configured = vault
                    .as_ref()
                    .is_some_and(|v| v.get(id).ok().flatten().is_some());
                ProviderConfigView {
                    id: id.clone(),
                    kind,
                    base_url: base,
                    configured,
                }
            })
            .collect()
    }

    fn set_key(&self, id: &str, key: &str) -> Result<(), String> {
        let mut vault = self.vault.lock().map_err(|e| e.to_string())?;
        let v = vault
            .as_mut()
            .ok_or("vault indisponível (HARNESSRS_VAULT_KEY ausente)")?;
        v.set(id, secrecy::SecretString::from(key.to_string()))
            .map_err(|e| e.to_string())
    }

    fn add_provider(&self, name: &str, kind: &str, base_url: &str) -> Result<(), String> {
        let kind = ProviderKind::parse(kind).ok_or_else(|| format!("kind inválido: {kind}"))?;
        {
            let mut cfg = self.config.lock().map_err(|e| e.to_string())?;
            cfg.add_custom(name, kind, base_url);
        }
        self.save()
    }

    fn remove_provider(&self, id: &str) -> Result<(), String> {
        {
            let mut cfg = self.config.lock().map_err(|e| e.to_string())?;
            cfg.remove(id);
        }
        self.save()
    }

    async fn test_connection(&self, id: &str) -> Result<String, String> {
        let (cfg, vault) = {
            let c = self.config.lock().map_err(|e| e.to_string())?.clone();
            let v = self.vault.lock().map_err(|e| e.to_string())?.clone();
            (c, v)
        };
        let Some(vault) = vault else {
            return Err("vault indisponível".into());
        };
        let router = ProviderRouter::new(&cfg, &vault).map_err(|e| e.to_string())?;
        let alias = ModelAlias::parse(&format!("{id}/probe")).map_err(|e| e.to_string())?;
        let resolved = router.resolve(&alias).map_err(|e| e.to_string())?;
        let models = resolved
            .provider
            .models()
            .await
            .map_err(|e| e.to_string())?;
        Ok(format!("{} modelos", models.len()))
    }

    fn export_template(&self, path: &str) -> Result<(), String> {
        let cfg = self.config.lock().map_err(|e| e.to_string())?;
        cfg.export_template(std::path::Path::new(path)).map_err(|e| e.to_string())
    }

    fn last_model(&self) -> Option<String> {
        self.config.lock().ok()?.ui.last_model.clone()
    }

    fn set_last_model(&self, alias: &str) -> Result<(), String> {
        {
            let mut cfg = self.config.lock().map_err(|e| e.to_string())?;
            cfg.ui.last_model = Some(alias.to_string());
        }
        self.save()
    }

    fn resolve(
        &self,
        alias: &ModelAlias,
    ) -> Result<(std::sync::Arc<dyn harness_core::provider_port::LlmProvider>, String), String> {
        let (cfg, vault) = {
            let c = self.config.lock().map_err(|e| e.to_string())?.clone();
            let v = self.vault.lock().map_err(|e| e.to_string())?.clone();
            (c, v)
        };
        let vault = vault.ok_or("vault indisponível (HARNESSRS_VAULT_KEY ausente)")?;
        let router = ProviderRouter::new(&cfg, &vault).map_err(|e| e.to_string())?;
        let resolved = router.resolve(alias).map_err(|e| e.to_string())?;
        Ok((resolved.provider, resolved.model))
    }

    async fn list_models(&self, id: &str) -> Result<Vec<String>, String> {
        let alias = ModelAlias::parse(&format!("{id}/probe")).map_err(|e| e.to_string())?;
        let (provider, _) = self.resolve(&alias)?;
        let models = provider.models().await.map_err(|e| e.to_string())?;
        Ok(models.into_iter().map(|m| m.id).collect())
    }
}
