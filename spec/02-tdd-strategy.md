# 02 — Estratégia TDD

**Regra de ouro: nenhum código de produção sem teste falhando antes (red → green → refactor).** Cada wave em 08-waves.md lista os testes que devem existir antes da implementação.

## Ferramentas (D5)

| Ferramenta | Uso |
|---|---|
| `cargo test` | testes unitários/doc-tests |
| `cargo nextest run` | runner padrão de CI e local (`just test`) |
| `insta` | snapshots de: render TUI (via TestBackend), SSE parsers, serialização de sessão, mensagens de erro |
| ratatui `TestBackend` | renderiza tela em buffer e asserta conteúdo/snapshot |
| `proptest` | parsers (SSE, markdown chunks), reducers (estado nunca inválido) |
| `wiremock` | testes de providers HTTP sem rede |
| `assert_cmd` + `predicates` | testes E2E do binário com `--mock` provider |

## Pirâmide

```
        ┌─────────────┐
        │  E2E (~10%) │  binário + MockProvider + PTY (rexpect) nas waves finais
        ├─────────────┤
        │ Integração  │  core+tools+providers sem TUI: turn completo com tool approval simulada
        │   (~30%)    │
        ├─────────────┤
        │  Unitários  │  reducers, parsers, sandbox policy, roteador, vault, grafo
        │   (~60%)    │
        └─────────────┘
```

## Padrões obrigatórios

1. **Snapshot de telas**: todo widget novo começa com um teste insta tipo:
   ```rust
   #[test]
   fn chat_screen_renders_streaming_message() {
       let state = fixture_state();
       let backend = TestBackend::new(80, 24);
       let mut term = Terminal::new(backend).unwrap();
       term.draw(|f| ChatScreen::draw(f, &state)).unwrap();
       insta::assert_snapshot!(term.backend());
   }
   ```
2. **Fakes antes de reais**: `MockProvider` (scenario-driven) e `FakeTool` são escritos na Wave 0 e são pré-requisito de todas as waves.
3. **Reducers puros**: `harnizator-tui/state/reduce.rs` — 100% coberto por testes de propriedade: `proptest!(qualquer sequência de Event nunca panica e mantém invariantes)`.
4. **Sem flakes de tempo**: core recebe `trait Clock`; testes usam `PausedClock` / `tokio::time::pause()`.
5. **Mutation coverage leve**: `cargo mutants` nos crates core/tools rodado semanalmente (não bloqueia PR).
6. **CI** (GitHub Actions): `fmt --check`, `clippy -D warnings`, `nextest run`, `cargo doc`, MSRV pinado.

## Task runner

`justfile` (ou `cargo xtask` se preferir puro cargo — decidir na Wave 0 kickoff):
- `just tdd` → `cargo watch -x 'nextest run'`
- `just test` / `just snap` (insta review) / `just e2e` / `just ci`

## Critérios de aceite transversais por wave
- `cargo nextest run` verde
- nenhum `unwrap()` fora de testes (clippy lint)
- cobertura dos crates `harnizator-core` e `harnizator-tools` ≥ 85% (llvm-cov)
- snapshots revisados (`cargo insta review`)
