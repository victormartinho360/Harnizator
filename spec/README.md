# Harnizator — Especificação Técnica

Harness de agentes AI **full-TUI**, escrito 100% em Rust, desenvolvido estritamente com **TDD**.

## Índice

| Doc | Conteúdo |
|-----|----------|
| [00-decisions.md](00-decisions.md) | Decisões de arquitetura (resultado da entrevista) |
| [01-architecture.md](01-architecture.md) | Arquitetura: workspace, crates, event loop, traits centrais |
| [02-tdd-strategy.md](02-tdd-strategy.md) | Estratégia TDD: pirâmide de testes, insta, nextest, TestBackend |
| [03-providers.md](03-providers.md) | Camada de providers LLM, vault de chaves, tela de configuração |
| [04-tools-sandbox.md](04-tools-sandbox.md) | Sistema de tools, aprovação interativa e modos de sandbox |
| [05-tui.md](05-tui.md) | Telas do TUI: chat, streaming, markdown, keybindings |
| [06-agent-graph.md](06-agent-graph.md) | Subagentes concorrentes e grafo visual navegável |
| [07-persistence.md](07-persistence.md) | Persistência de sessões/histórico (SQLite) |
| [08-waves.md](08-waves.md) | **Waves de implementação** — plano executor |
| [09-multi-ui.md](09-multi-ui.md) | **Arquitetura limpa**: contrato UiPort, múltiplas UIs plugáveis |

## Visão em uma frase

Um terminal onde você conversa com agentes LLM (Anthropic, OpenAI, Google, ou qualquer endpoint OpenAI-compatible), observa tools sendo executadas com aprovação interativa, spawna subagentes em paralelo e navega por eles num **grafo visual** de hierarquia com context injection — tudo testável de ponta a ponta sem sair do `cargo test`, e com **toda a funcionalidade desacoplada da interface** (arquitetura limpa / ports & adapters), permitindo múltiplas UIs (TUI, headless/CLI, futuras GUI/web) sobre o mesmo núcleo.
