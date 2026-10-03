# HarnessRS

Harness de agentes AI **full-TUI**, escrito 100% em Rust, desenvolvido com **TDD estrito** (toda feature nasce de um teste vermelho). Inspirado no DeepSeek Harness, com arquitetura limpa (ports & adapters) que permite múltiplas UIs sobre o mesmo núcleo.

## O que ele faz

- **Chat com streaming** token a token, com markdown renderizado (headings, bold, código inline/fenced, listas, blockquotes)
- **Tools de workspace** (`read_file`, `write_file`, `edit_file`, `bash`, `glob`, `grep`) com **aprovação interativa** e modos de sandbox: `read-only`, `write-only`, `only-flagged` (allowlist de comandos), `full-access`
- **Subagentes concorrentes**: o LLM pode chamar `spawn_agent`; você navega por eles no **grafo visual** (Ctrl+2), envia mensagens (`m`), injeta contexto (`i`), interrompe (`x`)
- **Múltiplos providers**: Anthropic, OpenAI e Google já embutidos; providers custom OpenAI-compatible via tela Providers (Ctrl+5) — chaves salvas criptografadas em `~/.config/harnessrs/vault.age`
- **Persistência**: sessões gravadas em SQLite (`~/.config/harnessrs/harness.db`), com crash recovery e replay de histórico (`harness-rs sessions` / `harness-rs resume <id>`)
- **Modo headless** para pipes/CI com cenários mock determinísticos

## Requisitos

- Rust stable (1.85+), toolchain com gcc
- Em Fedora Atomic, use um toolbox: `toolbox enter <sua-toolbox>`

## Rodando

```bash
cargo build --release -p harness-app

# TUI com mock (offline, sem chave de API)
./target/release/harness-rs --mock crates/harness-app/tests/fixtures/tool_chat.toml --tools

# TUI com provider real
export HARNESSRS_VAULT_KEY="sua-senha-do-vault"
./target/release/harness-rs --model anthropic/claude-sonnet-4-5
# Ctrl+5 → tecla 'k' no provider → cola a chave → 't' testa a conexão

# headless (um turno, imprime e sai)
./target/release/harness-rs --mock crates/harness-app/tests/fixtures/chat.toml "hello there"
```

### Subcomandos

| Comando | Descrição |
|---|---|
| `harness-rs` | abre o TUI (modo default) |
| `harness-rs <prompt>` | turno headless |
| `harness-rs sessions` | lista sessões persistidas |
| `harness-rs resume <id>` | imprime o transcript de uma sessão |

### Flags

| Flag | Default | Descrição |
|---|---|---|
| `--mock <arquivo.toml>` | — | provider mock scriptado (offline/TDD) |
| `--model <provider/modelo>` | — | alias do provider (ex.: `google/gemini-2.5-pro`) |
| `--tools` | off | habilita tools de workspace |
| `--sandbox <modo>` | `only-flagged` | `read-only`, `write-only`, `only-flagged`, `full-access` |
| `--no-store` | off | não persiste sessão |

## Teclas do TUI

| Tecla | Ação |
|---|---|
| `Enter` | envia mensagem |
| `Esc` | cancela geração / fecha modal / sai de modo msg/inject |
| `Ctrl+1` | chat · `Ctrl+2` grafo · `Ctrl+3` ajuda · `Ctrl+4` sessões · `Ctrl+5` providers |
| `Ctrl+C` | sai |
| No modal de aprovação | `y` aprova · `a` aprova e registra · `n`/`Esc` nega |
| No grafo | `hjkl`/setas navegam · `Enter` abre chat · `m` manda mensagem · `i` injeta contexto · `x` interrompe |
| Em sessions | `jk` navegam · `Enter` resume · `Esc` volta |
| Em providers | `jk` navegam · `k` define chave · `a` adiciona · `t` testa · `d` remove |

## Arquitetura

```
┌ adapters de entrada: harness-tui (ratatui) · harness-cli (headless)
├ adapters de saída:   harness-providers (HTTP/SSE, vault) · harness-tools (fs/shell) · harness-store (SQLite)
└ core:                harness-core — domínio puro, ZERO dependências de IO/TUI/HTTP
```

Regras (checadas em CI, `just ci`):

1. Toda funcionalidade vive no core por trás de um port (`LlmProvider`, `ToolPort`, `ApprovalPort`, `SessionStore`, `ProviderAdmin`)
2. UIs consomem só o core; adapters de infra nunca são importados por UIs
3. `harness-core` não depende de tokio/reqwest/ratatui/rusqlite/crossterm
4. Nenhum código sem teste vermelho antes (red → green → refactor)

## Desenvolvimento

```bash
cargo test --workspace          # toda a suíte (~40 binários de teste, incl. E2E via PTY)
just ci                          # fmt + clippy + test + doc + check-arch
just tdd                         # watch mode
just snap                        # revisar snapshots insta
cargo run -p harness-app -- --mock crates/harness-app/tests/fixtures/tool_chat.toml --tools
```

Mais detalhes de design e waves de implementação: ver [`spec/`](spec/README.md).

## Licença

MIT
