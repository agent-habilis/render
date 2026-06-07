//! End-to-end tests for the `.csv`/`.tsv` table format, driving the real `ah-r`
//! binary. These run with stdout captured (not a TTY), so the renderer uses its
//! deterministic 80-column, no-ANSI path.

use std::path::PathBuf;
use std::process::Command;

const AH_R: &str = env!("CARGO_BIN_EXE_ah-r");

fn unique_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |dur| dur.as_nanos())
}

fn temp_file(contents: &str, extension: &str) -> PathBuf {
    let name = format!("ah-r-table-{}-{}.{extension}", std::process::id(), unique_nanos());
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, contents).expect("write temp file");
    path
}

fn render(contents: &str, extension: &str) -> String {
    let path = temp_file(contents, extension);
    let output = Command::new(AH_R).arg(&path).output().expect("run ah-r");
    let _ = std::fs::remove_file(&path);
    assert!(
        output.status.success(),
        "ah-r failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf8 stdout")
}

#[test]
fn csv_renders_rounded_grid_with_header() {
    let out = render("name,role\nAda,Engineer\n", "csv");

    // Rounded outer corners.
    assert!(out.contains('╭') && out.contains('╮'), "top corners: {out}");
    assert!(out.contains('╰') && out.contains('╯'), "bottom corners: {out}");
    // A solid header/row separator (├ ─ ┼ ┤), i.e. simple separators between rows.
    assert!(out.contains('├') && out.contains('┼') && out.contains('┤'), "row rule: {out}");
    // Header and data made it through.
    assert!(out.contains("name") && out.contains("role"), "header: {out}");
    assert!(out.contains("Ada") && out.contains("Engineer"), "data: {out}");
}

#[test]
fn tsv_uses_tab_as_the_delimiter() {
    let out = render("city\tcountry\nReykjavík\tIceland\n", "tsv");
    assert!(out.contains("city") && out.contains("country"), "header: {out}");
    assert!(out.contains("Reykjavík") && out.contains("Iceland"), "data: {out}");
    assert!(out.contains('╭'), "rounded grid: {out}");
}

#[test]
fn quoted_field_with_comma_stays_one_cell() {
    let out = render("name,note\nAda,\"Smith, Jr.\"\n", "csv");
    assert!(out.contains("Smith, Jr."), "quoted comma cell preserved: {out}");
    // Two columns → exactly three vertical borders per content line.
    let header_line = out
        .lines()
        .find(|line| line.contains("name"))
        .expect("a header line");
    assert_eq!(
        header_line.matches('│').count(),
        3,
        "two columns means three '│': {header_line:?}"
    );
}

#[test]
fn ragged_rows_pad_to_a_rectangle() {
    // A short row and an over-long row are both normalized to the column count.
    let out = render("a,b,c\n1\n4,5,6,7\n", "csv");
    let pipes: Vec<usize> = out
        .lines()
        .filter(|line| line.contains('│'))
        .map(|line| line.matches('│').count())
        .collect();
    assert!(!pipes.is_empty(), "expected content rows: {out}");
    assert!(
        pipes.iter().all(|&count| count == pipes[0]),
        "every row should have the same border count, got {pipes:?}:\n{out}"
    );
}

#[test]
fn leading_bom_is_not_glued_to_the_first_header() {
    let out = render("\u{feff}id,value\n1,x\n", "csv");
    let header_line = out.lines().find(|line| line.contains("id")).expect("header");
    // The BOM must not survive in front of `id`.
    assert!(
        !header_line.contains('\u{feff}'),
        "BOM should be stripped: {header_line:?}"
    );
}
