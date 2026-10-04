# 00 — Decisões de Arquitetura (Registro da Entrevista)

Status de cada decisão: ✅ decidida na entrevista.

| # | Tema | Decisão |
|---|------|---------|
| D1 | Propósito | Harness de **agentes AI com provedores plugáveis** (não clone 1:1 do DSH) |
| D2 | Stack TUI | **ratatui + crossterm + tokio** |
| D3 | Providers | Camada interna de roteamento (`trait LlmProvider`) + **vault local de chaves** + tela de configuração com defaults prontos (Anthropic, OpenAI, Google) e providers custom (base_url + api_key) |
| D4 | Escopo MVP | Chat/streaming com markdown, tools com aprovação, subagentes concorrentes, persistência de sessão, **tela de grafo de agentes** |
| D5 | TDD/CI | `cargo test` + **insta** (snapshots) + **nextest** + `TestBackend` do ratatui |
| D6 | Layout | **Cargo workspace**: `harnizator-core`, `harnizator-providers`, `harnizator-tools`, `harnizator-tui`, `harnizator-app` |
| D7 | Grafo | Navegável no MVP: nós = agentes (hierarquia parent/child), ações por nó: focar, enviar mensagem, interromper, injetar contexto |
| D8 | Sandbox | Aprovação interativa + **modos**: `read-only`, `write-only`, `only-flagged` (allowlist de comandos), extensível a `full-access` |

## Decisões derivadas (propostas, revisáveis)

| # | Decisão | Justificativa |
|---|---------|---------------|
| D9 | Event bus interno via `tokio::mpsc` + `tokio::broadcast` | Desacopla agent loop do TUI; permite N consumidores (TUI, logger, persistência) |
| D10 | Streaming via SSE com `reqwest-streams`/`eventsource-stream` + `futures` | Padrão para Anthropic/OpenAI |
| D11 | Vault de chaves: arquivo criptografado com **age** em `~/.config/harnizator/vault.age`, fallback para OS keyring (`keyring` crate) | Sem dependência de servidor; chave da vault via env var ou keyring |
| D12 | Markdown rendering: **pulldown-cmark** → tokens → estilos ratatui | Sem highlighting de sintaxe na Wave 1 (tui-syntax-highlight na Wave 4) |
| D13 | Persistência: **rusqlite** (bundled) com migrations simples; sessões serializam eventos (event-sourcing-lite) | Replay de sessão vira grátis; histórico de tokens/custos auditável |
| D14 | Config: TOML em `~/.config/harnizator/config.toml` via `serde` + `figment` para precedência (file < env < CLI) | Padrão Rust CLI |
| D15 | Erros: `thiserror` em libs, `anyhow` só em `harnizator-app` | Ergonomia + APIs tipadas |
| D16 | Async runtime: tokio multi-thread; tasks de agente com `tokio::task::JoinSet`; cancelamento via `CancellationToken` | Padrão para subagentes concorrentes |
| D17 | Rotação de providers por alias (`model = "anthropic/claude-sonnet-4"`), resolvida na camada de roteamento | UX igual a ferramentas como aider/litellm |
| D18 | Grafo renderizado manualmente em ratatui (layout em camadas por profundidade); crate `petgraph` apenas para o modelo de dados, **não** para layout gráfico externo | Evita dep pesada de graphviz; testável com TestBackend |
