//! harness-providers: adapters de provedores LLM.
//!
//! Wave 0: o contrato `LlmProvider` + `MockProvider` (base do TDD).
//! Wave 1: Anthropic, OpenAI-compatible, router, vault.

pub mod mock;
pub mod provider;

pub use mock::MockProvider;
pub use provider::{ChatRequest, LlmProvider, ModelInfo, ProviderError, StreamChunk, StreamResult};
