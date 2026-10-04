//! Red tests: markdown → ratatui Text (spec/05, D12).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use harnizator_tui::markdown::render_markdown;

#[test]
fn headings_inline_code_and_bold_snapshot() {
    let text = render_markdown("# Title\n\nSome **bold** and `code` inline.");
    insta::assert_debug_snapshot!(text);
}

#[test]
fn fenced_code_block_and_list_snapshot() {
    let md = "```rust\nfn main() {}\n```\n\n- item a\n- item b\n";
    let text = render_markdown(md);
    insta::assert_debug_snapshot!(text);
}

#[test]
fn blockquote_gets_gutter() {
    let text = render_markdown("> quoted line");
    let dbg = format!("{text:?}");
    assert!(dbg.contains('│') || dbg.contains("> "));
}

#[test]
fn malformed_markdown_never_panics() {
    for md in ["# \n**unclosed", "```\nno close", "[bad](", "| a |", ""] {
        let _ = render_markdown(md);
    }
}
