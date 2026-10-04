# Harnizator

Harness de agentes AI **full-TUI**, escrito 100% em Rust, desenvolvido com **TDD estrito** (toda feature nasce de um teste vermelho). Inspirado no DeepSeek Harness, com arquitetura limpa (ports & adapters) que permite múltiplas UIs sobre o mesmo núcleo.

## O que ele faz

- **Chat com streaming** token a token, com markdown renderizado (headings, bold, código inline/fenced, listas, blockquotes)
- **Tools de workspace** (`read_file`, `write_file`, `edit_file`, `bash`, `glob`, `grep`) com **aprovação interativa** e modos de sandbox: `read-only`, `write-only`, `only-flagged` (allowlist de comandos), `full-access`
- **Subagentes concorrentes**: o LLM pode chamar `spawn_agent`; você navega por eles no **grafo visual** (Ctrl+2), envia mensagens (`m`), injeta contexto (`i`), interrompe (`x`)
- **Múltiplos providers**: Anthropic, OpenAI e Google já embutidos; providers custom OpenAI-compatible via tela Providers (Ctrl+5) — chaves salvas criptografadas em `~/.config/harnizator/vault.age`
- **Persistência**: sessões gravadas em SQLite (`~/.config/harnizator/harness.db`), com crash recovery e replay de histórico (`harnizator sessions` / `harnizator resume <id>`)
- **Modo headless** para pipes/CI com cenários mock determinísticos

## Requisitos

- Rust stable (1.85+), toolchain com gcc
- Em Fedora Atomic, use um toolbox: `toolbox enter <sua-toolbox>`

## Rodando

```bash
cargo build --release -p harnizator-app

# TUI com mock (offline, sem chave de API)
./target/release/harnizator --mock crates/harnizator-app/tests/fixtures/tool_chat.toml --tools

# TUI com provider real
export HARNIZATOR_VAULT_KEY="sua-senha-do-vault"
./target/release/harnizator --model anthropic/claude-sonnet-4-5
# Ctrl+5 → tecla 'k' no provider → cola a chave → 't' testa a conexão

# headless (um turno, imprime e sai)
./target/release/harnizator --mock crates/harnizator-app/tests/fixtures/chat.toml "hello there"
```

### Subcomandos

| Comando | Descrição |
|---|---|
| `harnizator` | abre o TUI (modo default) |
| `harnizator <prompt>` | turno headless |
| `harnizator sessions` | lista sessões persistidas |
| `harnizator resume <id>` | imprime o transcript de uma sessão |

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
| `Tab` / `Shift+Tab` | cicla as telas · `F1` chat · `F2` grafo · `F3` ajuda · `F4` sessões · `F5` providers · (`Ctrl+G` = grafo; Ctrl+dígito funciona em terminais com CSI-u) |
| `Ctrl+C` | sai |
| No modal de aprovação | `y` aprova · `a` aprova e registra · `n`/`Esc` nega |
| No grafo | `hjkl`/setas navegam · `Enter` abre chat · `m` manda mensagem · `i` injeta contexto · `x` interrompe |
| Em sessions | `jk` navegam · `Enter` resume · `Esc` volta |
| Em providers | `jk` navegam · `k` define chave · `a` adiciona · `t` testa · `d` remove |

## Arquitetura

```
┌ adapters de entrada: harnizator-tui (ratatui) · harnizator-cli (headless)
├ adapters de saída:   harnizator-providers (HTTP/SSE, vault) · harnizator-tools (fs/shell) · harnizator-store (SQLite)
└ core:                harnizator-core — domínio puro, ZERO dependências de IO/TUI/HTTP
```

Regras (checadas em CI, `just ci`):

1. Toda funcionalidade vive no core por trás de um port (`LlmProvider`, `ToolPort`, `ApprovalPort`, `SessionStore`, `ProviderAdmin`)
2. UIs consomem só o core; adapters de infra nunca são importados por UIs
3. `harnizator-core` não depende de tokio/reqwest/ratatui/rusqlite/crossterm
4. Nenhum código sem teste vermelho antes (red → green → refactor)

## Desenvolvimento

```bash
cargo test --workspace          # toda a suíte (~40 binários de teste, incl. E2E via PTY)
just ci                          # fmt + clippy + test + doc + check-arch
just tdd                         # watch mode
just snap                        # revisar snapshots insta
cargo run -p harnizator-app -- --mock crates/harnizator-app/tests/fixtures/tool_chat.toml --tools
```

Mais detalhes de design e waves de implementação: ver [`spec/`](spec/README.md).

## Licença

MIT
