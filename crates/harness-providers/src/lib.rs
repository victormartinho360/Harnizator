//! harness-providers: adapters de provedores LLM.
//!
//! Wave 0: contrato `LlmProvider` + `MockProvider` (base do TDD).
//! Wave 1: parsers SSE, Anthropic, OpenAI-compatible, router, vault, retry.

pub mod anthropic;
pub mod http;
pub mod mock;
pub mod openai;
pub mod provider;
pub mod router;
pub mod sse;
pub mod stream;
pub mod vault;

pub use anthropic::AnthropicProvider;
pub use mock::MockProvider;
pub use openai::OpenAiProvider;
pub use provider::{ChatRequest, LlmProvider, ModelInfo, ProviderError, StreamChunk, StreamResult};
pub use router::{ProviderConfig, ProviderRouter, ResolvedProvider, RouterError};
pub use vault::{Vault, VaultError};
