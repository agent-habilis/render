use std::io::IsTerminal;

use comfy_table::{Attribute, Cell, CellAlignment, ContentArrangement, Table};

/// comfy-table preset string: one glyph per [`comfy_table::TableComponent`], in
/// the order documented by the crate. We hand-roll it (rather than `UTF8_FULL`,
/// whose interior is dashed/dotted and whose header rule is a double line) to get
/// a *simple* solid single-line grid with rounded outer corners.
///
/// Order: left/right/top/bottom border, header (left ╶ rule ╶ mid ╶ right),
/// vertical / horizontal / middle interior, left/right/top/bottom border
/// intersections, then the four corners.
const ROUNDED_GRID: &str = "\
││──\
├─┼┤\
│─┼\
├┤┬┴\
╭╮╰╯";

/// Wrap column for stdout: the terminal width when stdout is a TTY (comfy-table
/// detects it), else a deterministic `80` so piped output is stable. Mirrors
/// [`crate::markdown`]'s policy.
const PIPED_WIDTH: u16 = 80;

/// Render a CSV/TSV document as a rounded-corner terminal table. `delimiter` is
/// the byte separating fields (`b','` for CSV, `b'\t'` for TSV).
pub(crate) fn table(input: &str, delimiter: u8) -> String {
    // Excel and friends prefix a UTF-8 BOM; the `csv` crate keeps it, which would
    // otherwise show up glued to the first header cell.
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);

    let rows = parse(input, delimiter);
    if rows.is_empty() {
        return String::new();
    }

    // A rectangular grid: pad short rows (and, when a data row is wider than the
    // header, the header too) with empty cells so comfy-table sees a uniform
    // column count.
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);

    let mut table = Table::new();
    table.load_preset(ROUNDED_GRID);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    if !std::io::stdout().is_terminal() {
        table.set_width(PIPED_WIDTH);
    }

    let mut iter = rows.into_iter();
    if let Some(header) = iter.next() {
        table.set_header(padded(header, columns).into_iter().map(header_cell));
    }
    for row in iter {
        table.add_row(padded(row, columns).into_iter().map(Cell::new));
    }

    table.to_string()
}

/// Parse the document into rows of sanitized fields. `flexible(true)` tolerates
/// ragged rows; `has_headers(false)` so we keep the first record and style it
/// ourselves.
fn parse(input: &str, delimiter: u8) -> Vec<Vec<String>> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(input.as_bytes());

    reader
        .records()
        .filter_map(Result::ok)
        .map(|record| record.iter().map(sanitize).collect())
        .collect()
}

/// Strip C0 control characters that would corrupt the grid, keeping `\n`/`\t`
/// (legitimate multi-line / tabbed cell content that comfy-table lays out).
fn sanitize(field: &str) -> String {
    field
        .chars()
        .filter(|&ch| ch == '\n' || ch == '\t' || !ch.is_control())
        .collect()
}

fn padded(mut row: Vec<String>, columns: usize) -> Vec<String> {
    row.resize(columns, String::new());
    row
}

/// Header cells are centered and bold so the top row reads as a header (the bold
/// styling renders only on a TTY, leaving piped output plain and deterministic;
/// the centering is structural and applies either way).
fn header_cell(text: String) -> Cell {
    Cell::new(text)
        .set_alignment(CellAlignment::Center)
        .add_attribute(Attribute::Bold)
}
