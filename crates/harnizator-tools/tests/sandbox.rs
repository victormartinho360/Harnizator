//! Red tests: SandboxPolicy — matriz modo × risco × flaglist (spec/04).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use harnizator_core::tool_port::{ToolCall, ToolDecision, ToolPort};
use harnizator_tools::{FlagPolicy, SandboxMode, ToolCtx, Toolbelt};

fn belt_with(mode: SandboxMode) -> (tempfile::TempDir, Toolbelt) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "data").unwrap();
    let belt = Toolbelt::new(ToolCtx::new(dir.path()).unwrap(), mode).unwrap();
    (dir, belt)
}

fn call(name: &str, args: serde_json::Value) -> ToolCall {
    ToolCall {
        id: "c1".into(),
        name: name.into(),
        args,
    }
}

#[test]
fn toolbelt_registers_all_builtins_with_specs() {
    let (_d, belt) = belt_with(SandboxMode::FullAccess);
    let names: Vec<_> = belt.spec_names();
    for expected in [
        "read_file",
        "write_file",
        "edit_file",
        "bash",
        "glob",
        "grep",
    ] {
        assert!(names.contains(&expected.to_string()), "faltou {expected}");
    }
    // specs devem ter JSON Schema para anunciar ao LLM
    assert!(belt.specs().iter().all(|s| s.input_schema.is_object()));
}

#[test]
fn read_only_denies_write_and_execute() {
    let (_d, belt) = belt_with(SandboxMode::ReadOnly);
    assert_eq!(
        belt.decide(&call("read_file", serde_json::json!({"path": "a.txt"}))),
        ToolDecision::Allow
    );
    assert!(matches!(
        belt.decide(&call(
            "write_file",
            serde_json::json!({"path": "a.txt", "content": "x"})
        )),
        ToolDecision::Deny { .. }
    ));
    assert!(matches!(
        belt.decide(&call("bash", serde_json::json!({"command": "ls"}))),
        ToolDecision::Deny { .. }
    ));
}

#[test]
fn write_only_allows_read_and_write_but_denies_execute() {
    let (_d, belt) = belt_with(SandboxMode::WriteOnly);
    assert_eq!(
        belt.decide(&call(
            "write_file",
            serde_json::json!({"path": "a.txt", "content": "x"})
        )),
        ToolDecision::Allow
    );
    assert_eq!(
        belt.decide(&call("glob", serde_json::json!({"pattern": "*"}))),
        ToolDecision::Allow
    );
    assert!(matches!(
        belt.decide(&call("bash", serde_json::json!({"command": "ls"}))),
        ToolDecision::Deny { .. }
    ));
}

#[test]
fn full_access_requires_interactive_approval() {
    let (_d, belt) = belt_with(SandboxMode::FullAccess);
    assert!(matches!(
        belt.decide(&call("read_file", serde_json::json!({"path": "a.txt"}))),
        ToolDecision::NeedsApproval { .. }
    ));
}

#[test]
fn only_flagged_allows_flagged_commands() {
    let mut flags = FlagPolicy::new();
    flags.add_group("rust", &["cargo *"]);
    let (_d, belt) = belt_with(SandboxMode::OnlyFlagged(flags));
    assert_eq!(
        belt.decide(&call("bash", serde_json::json!({"command": "cargo test"}))),
        ToolDecision::Allow
    );
    assert!(matches!(
        belt.decide(&call("bash", serde_json::json!({"command": "rm -rf /"}))),
        ToolDecision::NeedsApproval { .. }
    ));
}

#[test]
fn compound_commands_are_never_auto_allowed() {
    let mut flags = FlagPolicy::new();
    flags.add_group("rust", &["cargo *"]);
    let (_d, belt) = belt_with(SandboxMode::OnlyFlagged(flags));
    for cmd in [
        "cargo test && rm -rf /",
        "cargo test; true",
        "cargo test | tee x",
        "echo `whoami`",
    ] {
        assert!(
            matches!(
                belt.decide(&call("bash", serde_json::json!({"command": cmd}))),
                ToolDecision::NeedsApproval { .. }
            ),
            "composto deveria exigir aprovação: {cmd}"
        );
    }
}

#[test]
fn path_traversal_is_denied_even_in_full_access() {
    let (_d, belt) = belt_with(SandboxMode::FullAccess);
    for args in [
        serde_json::json!({"path": "../outside.txt"}),
        serde_json::json!({"path": "/etc/passwd"}),
    ] {
        let decision = belt.decide(&call("read_file", args));
        assert!(
            matches!(decision, ToolDecision::Deny { .. }),
            "escape deve ser Deny: {decision:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn symlink_escape_is_denied() {
    let (_d, belt) = belt_with(SandboxMode::FullAccess);
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.txt"), "shh").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret.txt"),
        belt.ctx().root().join("link.txt"),
    )
    .unwrap();
    let decision = belt.decide(&call("read_file", serde_json::json!({"path": "link.txt"})));
    assert!(matches!(decision, ToolDecision::Deny { .. }));
}

#[test]
fn unknown_tool_is_denied() {
    let (_d, belt) = belt_with(SandboxMode::FullAccess);
    assert!(matches!(
        belt.decide(&call("rm_tool", serde_json::json!({}))),
        ToolDecision::Deny { .. }
    ));
}

#[test]
fn glob_matcher_patterns() {
    assert!(harnizator_tools::flags::glob_match(
        "cargo *",
        "cargo build --release"
    ));
    assert!(harnizator_tools::flags::glob_match("git status", "git status"));
    assert!(!harnizator_tools::flags::glob_match(
        "git status",
        "git statuss"
    ));
    assert!(harnizator_tools::flags::glob_match(
        "git * *",
        "git log --oneline --graph"
    ));
    assert!(!harnizator_tools::flags::glob_match("cargo *", "ls cargo"));
}
