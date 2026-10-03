//! Red test E2E: `harness-rs --mock` (Wave 1 entregável).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use assert_cmd::Command;
use predicates::str::contains;

fn fixture() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/chat.toml").to_string()
}

#[test]
fn headless_mock_chat_streams_to_stdout() {
    Command::cargo_bin("harness-rs")
        .unwrap()
        .args(["--mock", &fixture(), "hello there"])
        .assert()
        .success()
        .stdout(contains("Hello world"));
}

#[test]
fn headless_mock_unmatched_prompt_fails() {
    Command::cargo_bin("harness-rs")
        .unwrap()
        .args(["--mock", &fixture(), "zzz nothing matches"])
        .assert()
        .failure()
        .stderr(contains("no scenario entry matched"));
}
