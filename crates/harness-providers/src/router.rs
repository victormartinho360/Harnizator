//! ProviderRouter: resolve aliases `provider/model` (spec/03, D17).

use std::collections::BTreeMap;
use std::sync::Arc;

use harness_core::ModelAlias;

use crate::anthropic::AnthropicProvider;
use crate::openai::OpenAiProvider;
use crate::provider::LlmProvider;
use crate::vault::Vault;

/// Tipo de protocolo do provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub enum ProviderKind {
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "openai-compatible")]
    OpenAiCompatible,
}

/// Entrada de configuração de um provider.
#[derive(Debug, Clone, serde::Deserialize)]
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
            other => Err(RouterError::UnknownKind(other.to_string())),
        }
    }

    fn base_url(&self, kind: ProviderKind) -> String {
        self.base_url.clone().unwrap_or_else(|| match kind {
            ProviderKind::Anthropic => crate::anthropic::DEFAULT_BASE_URL.to_string(),
            ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => {
                crate::openai::DEFAULT_BASE_URL.to_string()
            }
        })
    }
}

/// Configuração da seção `[providers]` do TOML.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ProviderConfig {
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderEntry>,
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
