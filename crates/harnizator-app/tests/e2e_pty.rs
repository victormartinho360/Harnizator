#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Red test E2E via PTY: sobe o TUI real, conversa, aprova tool, sai (spec/08-w7).

use std::process::Command;

fn scenario() -> String {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    // cenário com texto puro (sem tools): caminho mais estável em PTY
    format!("{dir}/tests/fixtures/chat.toml")
}

fn tool_scenario() -> String {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    format!("{dir}/tests/fixtures/tool_chat.toml")
}

/// Sobe o TUI em um PTY, manda "hello there\n", espera resposta mock, Ctrl+C.
#[test]
fn tui_boot_streams_reply_and_quits() {
    let bin = env!("CARGO_BIN_EXE_harnizator");
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c").arg(format!(
        "stty rows 24 cols 80 2>/dev/null; exec {} --mock {} --no-store",
        bin,
        scenario()
    ));
    let mut p = rexpect::session::spawn_command(cmd, Some(30_000)).unwrap();

    // status bar prova boot
    p.exp_string("harnizator").unwrap();
    // digita e envia
    p.send_line("hello there").unwrap();
    p.send_control('m').unwrap();
    // streaming chega em chunks separados por escapes ANSI
    p.exp_string("[assistant]").unwrap();
    p.exp_string("Hello").unwrap();
    p.exp_string("world").unwrap();
    // sai
    p.send_control('c').unwrap();
    p.exp_eof().unwrap();
}

/// Fluxo de aprovação: full-access pede y para read_file; resposta final segue.
#[test]
fn tui_tab_opens_graph_screen() {
    let bin = env!("CARGO_BIN_EXE_harnizator");
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c").arg(format!(
        "stty rows 24 cols 80 2>/dev/null; exec {} --mock {} --no-store",
        bin,
        scenario()
    ));
    let mut p = rexpect::session::spawn_command(cmd, Some(30_000)).unwrap();
    p.exp_string("harnizator").unwrap();
    p.send("z").unwrap();
    p.flush().unwrap();
    p.exp_string("z").unwrap(); // teclas chegam
    p.send("\x07").unwrap();
    p.flush().unwrap(); // Ctrl+G → grafo
    std::thread::sleep(std::time::Duration::from_millis(300));
    p.exp_string("agentes").unwrap();
    p.exp_string("root").unwrap();
    p.send_control('c').unwrap();
    p.exp_eof().unwrap();
}

#[test]
fn tui_approves_tool_call() {
    let bin = env!("CARGO_BIN_EXE_harnizator");
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("secret.txt"), "top secret!\n").unwrap();
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c").arg(format!(
        "stty rows 24 cols 80 2>/dev/null; exec {} --mock {} --tools --sandbox full-access --no-store",
        bin,
        tool_scenario()
    ))
    .current_dir(dir.path());
    let mut p = rexpect::session::spawn_command(cmd, Some(30_000)).unwrap();

    p.exp_string("harnizator").unwrap();
    p.send_line("what does secret.txt contain").unwrap();
    p.send_control('m').unwrap();
    // modal de aprovação aparece (evitamos non-ASCII que pode vir split)
    p.exp_string("aprova").unwrap();
    p.send_line("y").unwrap();
    p.exp_string("approved").unwrap();
    p.exp_string("secret!").unwrap();
    p.send_control('c').unwrap();
    p.exp_eof().unwrap();
}
