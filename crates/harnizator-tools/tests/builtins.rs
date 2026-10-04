//! Red tests: tools builtin (spec/04).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use harnizator_tools::builtins::*;
use harnizator_tools::{Tool, ToolCtx, ToolError};

fn ctx() -> (tempfile::TempDir, ToolCtx) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "line1\nline2\nline3\n").unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/b.rs"), "fn main() {}\nTODO: x\n").unwrap();
    let ctx = ToolCtx::new(dir.path()).unwrap();
    (dir, ctx)
}

#[tokio::test]
async fn read_file_returns_content() {
    let (_d, ctx) = ctx();
    let out = ReadFileTool
        .execute(serde_json::json!({"path": "a.txt"}), &ctx)
        .await
        .unwrap();
    assert!(out.content.contains("line1"));
    assert!(!out.truncated);
}

#[tokio::test]
async fn read_file_offset_limit_and_truncation() {
    let (_d, ctx) = ctx();
    let out = ReadFileTool
        .execute(
            serde_json::json!({"path": "a.txt", "offset": 2, "limit": 1}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(out.content.trim(), "line2");
    assert!(out.truncated, "deve sinalizar que há mais linhas");
}

#[tokio::test]
async fn read_file_missing_is_error() {
    let (_d, ctx) = ctx();
    let err = ReadFileTool
        .execute(serde_json::json!({"path": "nope.txt"}), &ctx)
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::Io(_)));
}

#[tokio::test]
async fn write_file_creates_and_edit_file_replaces_unique() {
    let (_d, ctx) = ctx();
    WriteFileTool
        .execute(
            serde_json::json!({"path": "n.txt", "content": "hello world"}),
            &ctx,
        )
        .await
        .unwrap();
    let out = EditFileTool
        .execute(
            serde_json::json!({"path": "n.txt", "old_string": "world", "new_string": "rust"}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(!out.is_error_flag());
    assert_eq!(
        std::fs::read_to_string(ctx.root().join("n.txt")).unwrap(),
        "hello rust"
    );
}

#[tokio::test]
async fn edit_file_requires_unique_match_unless_replace_all() {
    let (_d, ctx) = ctx();
    WriteFileTool
        .execute(
            serde_json::json!({"path": "m.txt", "content": "foo foo foo"}),
            &ctx,
        )
        .await
        .unwrap();
    let err = EditFileTool
        .execute(
            serde_json::json!({"path": "m.txt", "old_string": "foo", "new_string": "bar"}),
            &ctx,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::MultipleMatches(3)));
    EditFileTool
        .execute(
            serde_json::json!({"path": "m.txt", "old_string": "foo", "new_string": "bar", "replace_all": true}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(ctx.root().join("m.txt")).unwrap(),
        "bar bar bar"
    );
}

#[tokio::test]
async fn edit_file_no_match_errors() {
    let (_d, ctx) = ctx();
    let err = EditFileTool
        .execute(
            serde_json::json!({"path": "a.txt", "old_string": "zzz", "new_string": "y"}),
            &ctx,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::NoMatch));
}

#[tokio::test]
async fn glob_finds_files_under_root() {
    let (_d, ctx) = ctx();
    let out = GlobTool
        .execute(serde_json::json!({"pattern": "**/*.rs"}), &ctx)
        .await
        .unwrap();
    assert!(out.content.contains("src/b.rs"));
    assert!(!out.content.contains("a.txt"));
}

#[tokio::test]
async fn grep_finds_matches_with_line_numbers() {
    let (_d, ctx) = ctx();
    let out = GrepTool
        .execute(serde_json::json!({"pattern": "TODO"}), &ctx)
        .await
        .unwrap();
    assert!(out.content.contains("src/b.rs:2:"));
}

#[tokio::test]
async fn bash_captures_output_and_exit_code() {
    let (_d, ctx) = ctx();
    let out = BashTool
        .execute(serde_json::json!({"command": "echo hello; exit 3"}), &ctx)
        .await
        .unwrap();
    assert!(out.content.contains("hello"));
    assert_eq!(out.exit_code, Some(3));
}

#[tokio::test]
async fn bash_timeout_is_error_flagged_result() {
    let (_d, ctx) = ctx();
    let out = BashTool
        .execute(
            serde_json::json!({"command": "sleep 30", "timeout_ms": 100}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(out.is_error_flag());
    assert!(out.content.contains("timed out"));
}

#[tokio::test]
async fn bash_output_is_tail_truncated() {
    let (_d, ctx) = ctx();
    let cmd = "seq 1 5000";
    let out = BashTool
        .execute(serde_json::json!({"command": cmd}), &ctx)
        .await
        .unwrap();
    assert!(out.truncated, "saída grande deve truncar");
    assert!(out.content.contains("4999"), "tail preserva o fim");
}

#[tokio::test]
async fn bash_runs_in_workspace_root() {
    let (_d, ctx) = ctx();
    let out = BashTool
        .execute(serde_json::json!({"command": "pwd; ls a.txt"}), &ctx)
        .await
        .unwrap();
    assert!(out.content.contains("a.txt"));
}
