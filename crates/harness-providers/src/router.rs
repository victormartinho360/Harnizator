//! ProviderRouter: resolve aliases `provider/model` (spec/03, D17).

use std::collections::BTreeMap;
use std::sync::Arc;

use harness_core::ModelAlias;

use crate::anthropic::AnthropicProvider;
use crate::openai::OpenAiProvider;
use crate::provider::LlmProvider;
use crate::vault::Vault;

/// Tipo de protocolo do provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum ProviderKind {
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "openai-compatible")]
    OpenAiCompatible,
}

impl ProviderKind {
    /// Parse do nome usado na tela Providers / config TOML.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "anthropic" => Some(Self::Anthropic),
            "openai" => Some(Self::OpenAi),
            "openai-compatible" => Some(Self::OpenAiCompatible),
            _ => None,
        }
    }
}

/// Entrada de configuração de um provider.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ProviderEntry {
    /// Protocolo; se ausente, infere do nome ("anthropic", "openai").
    pub kind: Option<ProviderKind>,
    /// Sobrescreve a base_url padrão do protocolo.
    pub base_url: Option<String>,
}

impl ProviderEntry {
    /// Kind efetivo: explícito ou inferido do nome do provider.
    pub fn kind_named(&self, name: &str) -> Result<ProviderKind, RouterError> {
        if let Some(k) = self.kind {
            return Ok(k);
        }
        match name {
            "anthropic" => Ok(ProviderKind::Anthropic),
            "openai" => Ok(ProviderKind::OpenAi),
            "google" => Ok(ProviderKind::OpenAiCompatible),
            other => Err(RouterError::UnknownKind(other.to_string())),
        }
    }

    pub fn base_url(&self, kind: ProviderKind) -> String {
        self.base_url.clone().unwrap_or_else(|| match kind {
            ProviderKind::Anthropic => crate::anthropic::DEFAULT_BASE_URL.to_string(),
            ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => {
                crate::openai::DEFAULT_BASE_URL.to_string()
            }
        })
    }
}

/// Configuração da seção `[providers]` do TOML.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct ProviderConfig {
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderEntry>,
}

impl ProviderConfig {
    /// Defaults embutidos: anthropic, openai e google (sem chave) (spec/03).
    pub fn default_with_builtins() -> Self {
        let mut providers = BTreeMap::new();
        providers.insert(
            "anthropic".to_string(),
            ProviderEntry {
                kind: Some(ProviderKind::Anthropic),
                base_url: None,
            },
        );
        providers.insert(
            "openai".to_string(),
            ProviderEntry {
                kind: Some(ProviderKind::OpenAi),
                base_url: None,
            },
        );
        providers.insert(
            "google".to_string(),
            ProviderEntry {
                kind: Some(ProviderKind::OpenAiCompatible),
                base_url: Some(
                    "https://generativelanguage.googleapis.com/v1beta/openai".to_string(),
                ),
            },
        );
        Self { providers }
    }

    /// Carrega do disco; se o arquivo não existe, retorna os defaults.
    pub fn load_or_default(path: &std::path::Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(src) => {
                let mut cfg: Self = toml::from_str(&src).unwrap_or_default();
                // garante defaults mesmo com arquivo customizado
                for (k, v) in Self::default_with_builtins().providers {
                    cfg.providers.entry(k).or_insert(v);
                }
                cfg
            }
            Err(_) => Self::default_with_builtins(),
        }
    }

    /// Persiste a configuração em TOML.
    pub fn save(&self, path: &std::path::Path) -> Result<(), RouterError> {
        let src =
            toml::to_string_pretty(self).map_err(|e| RouterError::UnknownKind(e.to_string()))?;
        std::fs::write(path, src).map_err(|e| RouterError::UnknownKind(e.to_string()))
    }

    /// Adiciona provider custom (tela Providers, spec/03).
    pub fn add_custom(&mut self, name: &str, kind: ProviderKind, base_url: &str) {
        self.providers.insert(
            name.to_string(),
            ProviderEntry {
                kind: Some(kind),
                base_url: Some(base_url.to_string()),
            },
        );
    }

    /// Remove provider custom (mantém builtins que seriam re-carregados).
    pub fn remove(&mut self, name: &str) -> bool {
        self.providers.remove(name).is_some()
    }

    /// Views para a tela Providers: (id, kind, base_url, configured)?).
    pub fn views(&self, has_key: &dyn Fn(&str) -> bool) -> Vec<(String, String, String, bool)> {
        self.providers
            .iter()
            .map(|(id, e)| {
                let kind = e
                    .kind_named(id)
                    .map(|k| match k {
                        ProviderKind::Anthropic => "anthropic",
                        ProviderKind::OpenAi => "openai",
                        ProviderKind::OpenAiCompatible => "openai-compatible",
                    })
                    .unwrap_or("unknown")
                    .to_string();
                let base = e.base_url.clone().unwrap_or_else(|| {
                    e.base_url(e.kind_named(id).unwrap_or(ProviderKind::OpenAi))
                });
                (id.clone(), kind, base, has_key(id))
            })
            .collect()
    }
}

/// Resultado da resolução de um alias.
pub struct ResolvedProvider {
    pub provider: Arc<dyn LlmProvider>,
    pub model: String,
    pub base_url: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RouterError {
    #[error("unknown provider `{0}` (configure it in config.toml)")]
    UnknownProvider(String),
    #[error("provider `{0}` has no api key in the vault")]
    MissingKey(String),
    #[error("provider `{0}` needs an explicit `kind` in config.toml")]
    UnknownKind(String),
    #[error("vault error: {0}")]
    Vault(#[from] crate::vault::VaultError),
    #[error("provider error: {0}")]
    Provider(#[from] crate::provider::ProviderError),
}

/// Roteador de aliases para providers concretos.
pub struct ProviderRouter {
    config: ProviderConfig,
    vault: Vault,
}

impl ProviderRouter {
    pub fn new(config: &ProviderConfig, vault: &Vault) -> Result<Self, RouterError> {
        Ok(Self {
            config: config.clone(),
            vault: vault.clone(),
        })
    }

    /// Resolve `provider/model` → provider pronto para stream.
    pub fn resolve(&self, alias: &ModelAlias) -> Result<ResolvedProvider, RouterError> {
        let name = alias.provider().to_string();
        let entry = self
            .config
            .providers
            .get(&name)
            .ok_or_else(|| RouterError::UnknownProvider(name.clone()))?;
        let kind = entry.kind_named(&name)?;
        let base_url = entry.base_url(kind);
        let key = self
            .vault
            .get(&name)?
            .ok_or_else(|| RouterError::MissingKey(name.clone()))?;
        let provider: Arc<dyn LlmProvider> = match kind {
            ProviderKind::Anthropic => Arc::new(AnthropicProvider::new(&name, &base_url, key)?),
            ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => {
                Arc::new(OpenAiProvider::new(&name, &base_url, key)?)
            }
        };
        Ok(ResolvedProvider {
            provider,
            model: alias.model().to_string(),
            base_url,
        })
    }
}

#[cfg(test)]
mod tests_impl {
    // reexport para testes unitários internos futuros
}
