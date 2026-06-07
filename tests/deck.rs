//! End-to-end tests for the `.ah-r` presentation format, driving the real `ahr`
//! binary. A deck only runs in an interactive terminal, so the rendering path is
//! exercised through a pseudo-terminal.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

const AHR: &str = env!("CARGO_BIN_EXE_ahr");

fn unique_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |dur| dur.as_nanos())
}

fn temp_deck(contents: &str, extension: &str) -> PathBuf {
    let name = format!("ahr-test-{}-{}.{extension}", std::process::id(), unique_nanos());
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, contents).expect("write temp deck");
    path
}

#[test]
fn deck_outside_a_terminal_is_rejected() {
    let path = temp_deck("# One\n\n---\n\n# Two\n", "ah-r");
    let output = Command::new(AHR).arg(&path).output().expect("run ahr");
    let _ = std::fs::remove_file(&path);

    assert!(!output.status.success(), "a piped deck (no tty) must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("presentation"),
        "stderr should explain the deck needs a terminal, got: {stderr}"
    );
}

#[test]
fn unsupported_extension_is_rejected() {
    let path = temp_deck("# not a deck\n", "txt");
    let output = Command::new(AHR).arg(&path).output().expect("run ahr");
    let _ = std::fs::remove_file(&path);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported"), "got: {stderr}");
}

#[test]
fn deck_renders_slides_and_quits_in_a_pty() {
    let path = temp_deck("# Slide One\n\n---\n\n# Slide Two\n", "ah-r");

    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 30,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open pty");

    let mut cmd = CommandBuilder::new(AHR);
    cmd.arg(&path);
    cmd.env("TERM", "xterm-256color");
    let mut child = pair.slave.spawn_command(cmd).expect("spawn ahr");
    drop(pair.slave);

    let output = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&output);
    let mut reader = pair.master.try_clone_reader().expect("clone reader");
    let reader_thread = std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        while let Ok(read) = reader.read(&mut chunk) {
            if read == 0 {
                break;
            }
            sink.lock().expect("lock output").extend_from_slice(&chunk[..read]);
        }
    });

    let mut writer = pair.master.take_writer().expect("take writer");
    std::thread::sleep(Duration::from_millis(700));
    writer
        .write_all(b"\x1b[C")
        .and_then(|()| writer.flush())
        .expect("send next-slide key");
    std::thread::sleep(Duration::from_millis(400));
    writer.write_all(b"q").and_then(|()| writer.flush()).expect("send quit");

    // Kill if `q` is ever ignored, so a regression cannot hang the suite.
    let mut killer = child.clone_killer();
    let watchdog = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(5));
        let _ = killer.kill();
    });

    let status = child.wait().expect("wait for ahr");
    drop(pair.master);
    reader_thread.join().expect("join reader");
    let _ = watchdog.join();
    let _ = std::fs::remove_file(&path);

    assert!(status.success(), "deck should exit cleanly on `q`, got {status:?}");
    let bytes = output.lock().expect("lock output");
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("Slide One"), "first slide text should render");
    assert!(text.contains("1/2"), "footer should show the page indicator");
}
