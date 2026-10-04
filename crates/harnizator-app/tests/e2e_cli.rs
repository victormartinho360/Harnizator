//! Red test E2E: `harnizator --mock` (Wave 1 entregável).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use assert_cmd::Command;
use predicates::str::contains;

fn fixture() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/chat.toml").to_string()
}

#[test]
fn headless_mock_chat_streams_to_stdout() {
    Command::cargo_bin("harnizator")
        .unwrap()
        .args(["--mock", &fixture(), "hello there"])
        .assert()
        .success()
        .stdout(contains("Hello world"));
}

#[test]
fn headless_mock_unmatched_prompt_fails() {
    Command::cargo_bin("harnizator")
        .unwrap()
        .args(["--mock", &fixture(), "zzz nothing matches"])
        .assert()
        .failure()
        .stderr(contains("no scenario entry matched"));
}

#[test]
fn headless_agent_with_tools_reads_workspace_file() {
    let scenario =
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tool_chat.toml").to_string();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("secret.txt"), "top secret!\n").unwrap();

    Command::cargo_bin("harnizator")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "--mock",
            &scenario,
            "--tools",
            "--sandbox",
            "full-access",
            "what does secret.txt contain",
        ])
        .assert()
        .success()
        .stdout(contains("The file says: top secret!"))
        .stderr(contains("[tool] read_file requested"))
        .stderr(contains("approved"));
}
