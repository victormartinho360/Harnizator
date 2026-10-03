//! Port `LlmProvider` — definido no core (ports & adapters) e re-exportado
//! aqui por conveniência para os adapters deste crate.

pub use harness_core::provider_port::{
    ChatRequest, LlmProvider, ModelInfo, ProviderError, StreamChunk, StreamResult,
};
