//! Alias de modelo no formato `provider/model`.

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelAliasError {
    #[error("model alias must be in the form `provider/model`")]
    MissingSeparator,
    #[error("model alias has an empty provider or model")]
    EmptySegment,
}

/// Alias `provider/model` resolvido pelo `ProviderRouter` (Wave 1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ModelAlias {
    provider: String,
    model: String,
}

impl ModelAlias {
    pub fn parse(raw: &str) -> Result<Self, ModelAliasError> {
        let (provider, model) = raw
            .split_once('/')
            .ok_or(ModelAliasError::MissingSeparator)?;
        if provider.is_empty() || model.is_empty() {
            return Err(ModelAliasError::EmptySegment);
        }
        Ok(Self {
            provider: provider.to_string(),
            model: model.to_string(),
        })
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn model(&self) -> &str {
        &self.model
    }
}

impl std::fmt::Display for ModelAlias {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.provider, self.model)
    }
}

impl TryFrom<String> for ModelAlias {
    type Error = ModelAliasError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<ModelAlias> for String {
    fn from(alias: ModelAlias) -> Self {
        alias.to_string()
    }
}
