# 05 — TUI: Telas, Streaming e Markdown

## Shell do app

- Layout raiz: barra de status (modelo, sandbox mode, custo/tokens da sessão, nº de agentes) + área de tela ativa + barra de input contextual.
- Telas (stack-based router, `Screen` enum): `Chat`, `Graph`, `Providers`, `Sessions`, `Help`. Tecla global `Ctrl+1..5` troca de tela; `?` abre help.

## Tela Chat

- Lista de mensagens (user/assistant/tool) com scroll, auto-follow desligável (PageUp desativa, `End` reativa).
- Streaming: `AssistantDelta` appenda ao bloco corrente; spinner "thinking" até primeiro delta.
- Blocos de tool collapsáveis: `Tab` expande/colapsa; dialog de aprovação sobreposto (modal).
- Input multi-linha (`tui-textarea`): `Enter` envia, `Shift+Enter` quebra linha, `Esc` interrompe geração (`CancellationToken`).

## Markdown (D12)

- `pulldown-cmark` → eventos → `ratatui::text::Text` com estilos: headings (bold + cor), code inline (`bg`), fenced blocks (box com borda + label da linguagem), listas, links (underline), blockquotes.
- Fenced blocks ganham ação `y` (copy via `arboard`) quando focados.
- Syntax highlighting: Wave 4 (avaliar `syntect` vs tree-sitter; fora do MVP por peso de build).
- **Re-render incremental**: o parse roda por chunk de streaming com checkpointing — parser reutiliza estado (custom) ou re-parse com janela deslizante; decisão na Wave 3 com benchmark (critério: p95 < 2ms por delta em doc de 64KB).

## Tela Graph (visão geral; detalhe em 06)
- Renderiza hierarquia de agentes; teclas `h/j/k/l` ou setas navegam; `Enter` foca o chat do agente.

## Tela Sessions
- Lista sessões persistidas (título auto-gerado, data, modelo, custo); `Enter` resume, `d` deleta (confirmação), `n` nova.

## Keybindings globais (draft — finalizar na Wave 3)
| Tecla | Ação |
|---|---|
| `Ctrl+1..5` | trocar tela |
| `Esc` | cancelar geração / fechar modal |
| `Ctrl+C` | sair (confirma se há agente ativo) |
| `y/n/a/e` | dialog de aprovação |
| `?` | help contextual |

## Testes exigidos (antes da impl — Waves 3/4)
1. Snapshot insta de cada tela em ≥2 tamanhos (80x24, 120x40) e estados (vazio, streaming, erro, modal de aprovação).
2. Reducer de input: sequências de teclas → intents corretos (proptest).
3. Markdown: fixtures de documentos → snapshot do `Text` renderizado; propriedade "nunca panica em markdown malformado".
4. Auto-follow: PageUp/End mudam modo de scroll corretamente.
5. Resize: nenhum panics; layout `Constraint` testado com area degenerada (0x0, 1x1).
