//! Port de administração de providers (a tela Providers depende só disto).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConfigView {
    pub id: String,
    pub kind: String,
    pub base_url: String,
    pub configured: bool,
}

/// Port de saída: CRUD de providers + testes de conexão (spec/03, Wave 6).
#[async_trait::async_trait]
pub trait ProviderAdmin: Send + Sync {
    fn views(&self) -> Vec<ProviderConfigView>;
    fn set_key(&self, id: &str, key: &str) -> Result<(), String>;
    fn add_provider(&self, name: &str, kind: &str, base_url: &str) -> Result<(), String>;
    fn remove_provider(&self, id: &str) -> Result<(), String>;
    async fn test_connection(&self, id: &str) -> Result<String, String>;
}
