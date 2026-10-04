# 08 — Waves de Implementação

Cada wave: **(1) escrever os testes listados → (2) vermelho → (3) implementar → (4) verde + refactor → (5) `just ci` verde**. Nenhuma wave começa sem a anterior mergeada.

## Wave 0 — Fundação (½–1 dia)
**Escopo**: workspace, justfile/xtask, CI, lints, `harnizator-core` skeleton com tipos e `Event`.
- Testes: skeleton compila; `just ci` roda; exemplo de snapshot insta funcionando; `MockProvider` básico + cenario TOML.
- Entregável: `harnizator-core`, `harnizator-providers::MockProvider`, pipeline CI verde.

## Wave 1 — Providers & Vault
**Docs**: 03-providers.md.
- Testes 03 → parsers SSE (fixtures + proptest), router, vault, wiremock (auth/429/cut).
- Impl: `LlmProvider` trait, Anthropic + OpenAI-compatible, vault age, retry/backoff.
- Entregável: chat **UI headless formal (adapter `harnizator-cli`)** falando com provider real e mock: `harnizator --headless --mock ...`. Essa UI é a primeira prova do desacoplamento (ver 09-multi-ui.md).

## Wave 2 — Tools & Sandbox
**Docs**: 04-tools-sandbox.md.
- Testes 04 → policy matrix, path escape, flaglist, auditoria.
- Impl: 6 tools builtin, `SandboxPolicy`, loop de aprovação no core (approval resolvida por callback injetado — headless auto-aprova).
- Entregável: agent loop completo headless com tool use + aprovação programática.

## Wave 3 — TUI Core: Chat & Streaming
**Docs**: 05-tui.md.
- Testes 05 → snapshots de telas, reducers, markdown (fixtures + proptest), auto-follow, resize.
- Impl: app shell, Chat screen, input multi-linha, modal de aprovação, markdown incremental, `Esc` cancel.
- Entregável: **MVP utilizável**: chat streaming com tools aprováveis no TUI. TUI passa na **suite de contrato de UI** e o relatório de paridade TUI↔CLI (`just parity`) entra no `just ci` (ver 09-multi-ui.md).

## Wave 4 — Subagentes & Grafo
**Docs**: 06-agent-graph.md.
- Testes 06 → registry/invariantes, herança de sandbox, snapshots do grafo (1/5/50 nós), ações por nó, context injection, concurrency cap.
- Impl: `spawn_agent` tool, registry + JoinSet/CancellationToken, Graph screen navegável, painel de atividade, context injection, pan/zoom.
- Entregável: orquestração multi-agente completa no TUI.
- Plus (mesma wave): syntax highlighting em code fences, se benchmark passar (critério em 05-tui.md).

## Wave 5 — Persistência & Sessões
**Docs**: 07-persistence.md.
- Testes 07 → migrations, roundtrip, crash recovery, burst writer, snapshots Sessions.
- Impl: rusqlite + WAL, writer task, Sessions screen, resume, título/custo.
- Entregável: sessões sobrevivem a restart e crash.

## Wave 6 — Tela de Providers no TUI & Polish
**Docs**: 03 (tela), 05.
- Testes: snapshots do form de provider, fluxo teclado, test-connection com wiremock.
- Impl: Provider Setup screen completa, `t` testa conexão, help contextual on boarding em primeiro run (vault vazio → abre setup).
- Entregável: nunca editar TOML na mão para configurar provider.

## Wave 7 — E2E & Hardening
- Testes E2E via PTY (`rexpect`): boot → mock chat → tool approval → spawn agente → sair; suite `--mock` em CI.
- Hardening: uphold de `max_*` limits, redação de segredos em logs (`tracing` + secrecy), `cargo audit`, fuzz leve dos parsers (`cargo fuzz`, alvo SSE).
- Entregável: release v0.1.0 com install via `cargo install`.

## Waves futuras (backlog, fora do v0.1)
- Google provider nativo (se não coberto por openai-compatible), syntax highlight avançado, plugins de tools via WASM (`wasmtime`), MCP client, export de sessão (markdown/JSON), métricas de custo por projeto, temas.
