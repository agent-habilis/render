use std::process::Command;

const AH_R: &str = env!("CARGO_BIN_EXE_ah-r");

// Only the not-a-terminal guard is exercised: the successful inline render
// anchors its viewport with a cursor-position query a captured pipe never
// answers, so the actual draw is verified manually on a real terminal.
#[test]
fn image_outside_a_terminal_is_rejected() {
    let img = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/sample.png");
    // `output()` captures stdout/stderr through pipes, so neither is a TTY.
    let output = Command::new(AH_R).arg(img).output().expect("run ah-r");

    assert!(!output.status.success(), "a piped image (no tty) must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("image") && stderr.contains("interactive terminal"),
        "stderr should explain the image needs a terminal, got: {stderr}"
    );
}

#[test]
fn unsupported_extension_lists_images() {
    let output = Command::new(AH_R)
        .arg("nope.bmp")
        .output()
        .expect("run ah-r");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported"), "got: {stderr}");
    assert!(stderr.contains(".png"), "error should list image formats, got: {stderr}");
}
