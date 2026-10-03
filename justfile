# HarnessRS task runner
# Em ambientes sem toolchain local (sistema atômico), use: toolbox run -c c-env bash -lc 'just <task>'
# ou instale rustup+gcc na sua distro.

# TDD loop: testa em watch mode (requer cargo-watch)
tdd:
    cargo watch -c -x "nextest run" || cargo watch -c -x test

# Suíte completa (nextest se disponível, senão cargo test)
test:
    cargo nextest run --workspace 2>/dev/null || cargo test --workspace

# Testes E2E (Wave 7)
e2e:
    cargo test --workspace --test '*' -- --ignored

# Revisar snapshots insta
snap:
    cargo insta review

# Pipeline completo de CI
ci: fmt lint test doc check-arch
    @echo "CI green ✅"

fmt:
    cargo fmt --all -- --check

lint:
    cargo clippy --workspace --all-targets -- -D warnings

doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# Guarda arquitetural: o core não pode depender de IO/TUI/HTTP (ver spec/09-multi-ui.md)
check-arch:
    #!/usr/bin/env bash
    set -euo pipefail
    forbidden="tokio|reqwest|ratatui|crossterm|rusqlite"
    # apenas a seção [dependencies] conta (dev-deps de teste são permitidas)
    if awk '/^\[dependencies\]/{f=1;next} /^\[/{f=0} f' crates/harness-core/Cargo.toml | grep -E "^($forbidden)"; then
        echo "❌ harness-core tem dependência proibida (ver spec/09-multi-ui.md)" >&2
        exit 1
    fi
    echo "✅ arquitetura limpa ok"
