# 03 — Providers, Roteamento e Vault

## Modelo de configuração (D3, D17)

```toml
# ~/.config/harnessrs/config.toml
default_model = "anthropic/claude-sonnet-4-5"

[providers.anthropic]
kind = "anthropic"
# api_key vem do vault; base_url opcional
[providers.openai]
kind = "openai"
[providers.google]
kind = "google"
[providers.meu-proxy]
kind = "openai-compatible"
base_url = "http://localhost:8080/v1"
```

Defaults embutidos: `anthropic`, `openai`, `google` aparecem pré-criados (sem chave) na tela de setup.

## Tela de Provider Setup (MVP)

- Lista de providers (defaults + custom), status: `● configurado` / `○ sem chave`.
- Ações: `a` adicionar provider custom (form: nome, kind, base_url), `e` editar, `k` definir chave (input mascarado → vault), `t` testar conexão (`models()`), `d` remover custom.
- Teste de conexão exibe latência + nº de modelos; resultado é snapshotável.

## Vault (D11)

- Arquivo `~/.config/harnessrs/vault.age`, mapa `provider_id → api_key`.
- Chave de criptografia: OS keyring (`keyring` crate, service `harnessrs`); fallback `HARNESSRS_VAULT_KEY` env (útil em CI/headless).
- API: `Vault::get(id) -> Result<SecretString>`, `set`, `delete`, `list_ids`. Nunca loga segredos (Debug redacted via `secrecy`).

## Streaming

- `StreamChunk`: `TextDelta | ToolUseDelta | MessageStart | MessageStop | Usage | Error`.
- Parsers SSE por provider normalizam para `StreamChunk`. **Parsers são funções puras: `&[u8] → Vec<Result<SseEvent>>`** — alvo de proptest com fixtures gravados de respostas reais (sanitizadas).

## MockProvider (determinístico, TDD)

```rust
MockProvider::from_scenario(scenario: Scenario)
// Scenario = sequência de Turn scriptados: deltas com delays opcionais,
// tool_calls, erros injetáveis (timeout, 429, mid-stream drop)
```
Carregado por `--mock tests/scenarios/multi_tool.toml`. Usado em testes de integração/E2E e para desenvolvimento offline.

## Testes exigidos (antes da impl — ver Wave 1)
1. Router: resolve alias, erro para alias desconhecido, override de base_url.
2. Vault: roundtrip set/get, arquivo não-legível por outros (perm 0600), redação em Debug.
3. Parser SSE anthropic/openai/google: fixtures → snapshots insta; proptest de chunking arbitrário de bytes.
4. Wiremock: 401 → erro tipado `Auth`, 429 → retry com backoff (máx 3, jitter), stream cortado → `StreamInterrupted`.
5. Tela de setup: snapshots do form, fluxo teclado completo (reducers).
