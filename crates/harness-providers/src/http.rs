//! HTTP: POST de streaming com retry/backoff (spec/03).

use std::time::Duration;

use crate::provider::ProviderError;

/// Tentativas totais (1 inicial + retries) para 429/5xx.
pub const MAX_ATTEMPTS: u32 = 3;

/// Executa `build()` (fábrica de request) com retry em 429 e 5xx.
///
/// 401/403 → `Auth`; 429 exaurido → `RateLimited`; 5xx exaurido → `Stream`.
pub async fn send_with_retry(
    build: impl Fn() -> reqwest::RequestBuilder,
) -> Result<reqwest::Response, ProviderError> {
    let mut last_status = None;
    for attempt in 0..MAX_ATTEMPTS {
        if attempt > 0 {
            backoff_sleep(attempt).await;
        }
        let resp = build()
            .send()
            .await
            .map_err(|e| ProviderError::Stream(e.to_string()))?;
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        last_status = Some(status);
        match status.as_u16() {
            401 | 403 => return Err(ProviderError::Auth),
            429 => continue,
            s if (500..600).contains(&s) => continue,
            s => return Err(ProviderError::Stream(format!("unexpected HTTP status {s}"))),
        }
    }
    match last_status.map(|s| s.as_u16()) {
        Some(429) => Err(ProviderError::RateLimited),
        _ => Err(ProviderError::Stream("max retries exhausted".to_string())),
    }
}

async fn backoff_sleep(attempt: u32) {
    let base_ms = 50u64 << (attempt - 1);
    let jitter = rand::random_range(0..base_ms.max(1));
    tokio::time::sleep(Duration::from_millis(base_ms + jitter)).await;
}
