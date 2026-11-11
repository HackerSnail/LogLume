//! Column-preserving reconstruction of an OCR result.
//!
//! Every local OCR engine reports word-level boxes (Windows.Media.Ocr words,
//! macOS Vision observations, Tesseract TSV, PP-OCR detection boxes), but none
//! of them reports "this is column 3". This module turns a flat list of word
//! boxes into a row/column grid so a screenshot of a table keeps its columns
//! and a caller can pull one column out.
//!
//! The reconstruction is engine independent on purpose: swapping OCR engines
//! (OS built-in vs. a bundled ONNX model) must not change how columns are
//! detected, so this stays a dependency-free part of the core engine while
//! the engine itself lives in the app shell.
//!
//! Algorithm:
//!   1. group words into rows by vertical overlap;
//!   2. find x-axis gaps that no word crosses and that repeat across rows —
//!      those are the column separators;
//!   3. assign every word to the column holding its horizontal center;
//!   4. a cell is the words of one row inside one column, joined by a space.

/// One recognized word with its pixel box in the source image.
#[derive(Debug, Clone, PartialEq)]
pub struct WordBox {
    /// Recognized text (trimmed by the caller).
    pub text: String,
    /// Left edge in pixels.
    pub x: i32,
    /// Top edge in pixels.
    pub y: i32,
    /// Width in pixels.
    pub w: i32,
    /// Height in pixels.
    pub h: i32,
    /// Engine confidence in 0..=1. `None` when the engine does not report one
    /// (Windows.Media.Ocr has no per-word confidence at all).
    pub conf: Option<f32>,
}

impl WordBox {
    /// Horizontal span as `[start, end)`.
    pub fn x_range(&self) -> (i32, i32) {
        (self.x, self.x + self.w.max(0))
    }

    /// Vertical span as `[start, end)`.
    pub fn y_range(&self) -> (i32, i32) {
        (self.y, self.y + self.h.max(0))
    }

    /// Horizontal center, used for column assignment.
    pub fn x_center(&self) -> i32 {
        self.x + self.w.max(0) / 2
    }
}

/// Tuning knobs for the row/column reconstruction.
#[derive(Debug, Clone, Copy)]
pub struct GridOptions {
    /// Minimum width in pixels of an x-axis gap before it may separate columns.
    pub min_gap_px: i32,
    /// A gap must be at least this multiple of the median word width. Keeps the
    /// even word spacing of a log line from being read as column separators.
    pub gap_factor: f32,
    /// Minimum number of rows a gap must appear in to count as a separator.
    /// `0` means "auto": at least 2 rows and 30% of all rows.
    pub min_rows_for_split: usize,
}

impl Default for GridOptions {
    fn default() -> Self {
        GridOptions {
            min_gap_px: 6,
            gap_factor: 1.5,
            min_rows_for_split: 0,
        }
    }
}

/// One table cell: the words of a row inside one column.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cell {
    /// Cell text, words joined by a single space.
    pub text: String,
    /// Indices into the source word list, in reading order.
    pub words: Vec<usize>,
}

/// Horizontal extent of one detected column.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColumnSpan {
    /// Left boundary in pixels.
    pub x0: i32,
    /// Right boundary in pixels (exclusive).
    pub x1: i32,
}

/// A reconstructed table: rows of cells plus the column spans they came from.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Grid {
    /// Rows in reading order (top to bottom).
    pub rows: Vec<Vec<Cell>>,
    /// Column spans, left to right.
    pub columns: Vec<ColumnSpan>,
}

impl Grid {
    /// Number of detected columns.
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// Number of detected rows.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Cell text of one column, one entry per row (empty for missing cells).
    pub fn column_values(&self, col: usize) -> Vec<String> {
        if col >= self.columns.len() {
            return Vec::new();
        }
        self.rows
            .iter()
            .map(|row| row.get(col).map(|c| c.text.clone()).unwrap_or_default())
            .collect()
    }

    /// One column as newline separated text — ready to paste into the editor.
    pub fn column_text(&self, col: usize) -> String {
        self.column_values(col).join("\n")
    }

    /// The whole grid as tab separated rows (machine readable).
    pub fn to_tsv(&self) -> String {
        self.rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|c| c.text.replace('\t', " "))
                    .collect::<Vec<_>>()
                    .join("\t")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The whole grid as space padded columns — keeps the layout visible in a
    /// plain text editor.
    pub fn to_aligned_text(&self) -> String {
        let cols = self.column_count();
        if cols == 0 {
            return String::new();
        }
        let mut widths = vec![0usize; cols];
        for row in &self.rows {
            for (width, cell) in widths.iter_mut().zip(row.iter()) {
                let len = cell.text.chars().count();
                if len > *width {
                    *width = len;
                }
            }
        }
        self.rows
            .iter()
            .map(|row| {
                let mut parts: Vec<String> = Vec::with_capacity(cols);
                for (i, width) in widths.iter().enumerate().take(cols) {
                    let text = row.get(i).map(|c| c.text.as_str()).unwrap_or("");
                    let pad = width.saturating_sub(text.chars().count());
                    parts.push(format!("{}{}", text, " ".repeat(pad)));
                }
                parts.join(" ").trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Group words into rows by vertical overlap.
fn group_rows(words: &[WordBox]) -> Vec<Vec<usize>> {
    let mut order: Vec<usize> = (0..words.len()).collect();
    order.sort_by_key(|&i| (words[i].y, words[i].x));
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut cur: Vec<usize> = Vec::new();
    let mut band: (i32, i32) = (0, 0);
    for &i in &order {
        let w = &words[i];
        if w.text.trim().is_empty() {
            continue;
        }
        let (wy0, wy1) = w.y_range();
        if cur.is_empty() {
            cur.push(i);
            band = (wy0, wy1);
            continue;
        }
        // Same row when the word overlaps the current band on at least half of
        // its own height — tolerant of mixed font sizes in one line.
        let overlap = (wy1.min(band.1) - wy0.max(band.0)).max(0);
        let need = (w.h.max(1) as f32 * 0.5).ceil() as i32;
        if overlap >= need {
            cur.push(i);
            band = (band.0.min(wy0), band.1.max(wy1));
        } else {
            rows.push(std::mem::take(&mut cur));
            cur.push(i);
            band = (wy0, wy1);
        }
    }
    if !cur.is_empty() {
        rows.push(cur);
    }
    rows
}

/// Find x-axis gaps that no word crosses and that repeat across rows.
fn find_separators(rows: &[Vec<usize>], words: &[WordBox], opts: &GridOptions) -> Vec<i32> {
    // Merge all word x-intervals to get the covered stretches.
    let mut spans: Vec<(i32, i32)> = words
        .iter()
        .filter(|w| !w.text.trim().is_empty())
        .map(|w| w.x_range())
        .filter(|(a, b)| b > a)
        .collect();
    if spans.is_empty() {
        return Vec::new();
    }
    spans.sort();
    let mut merged: Vec<(i32, i32)> = Vec::with_capacity(spans.len());
    for (a, b) in spans {
        match merged.last_mut() {
            Some(last) if a <= last.1 => {
                if b > last.1 {
                    last.1 = b;
                }
            }
            _ => merged.push((a, b)),
        }
    }

    let min_rows = if opts.min_rows_for_split > 0 {
        opts.min_rows_for_split
    } else if rows.len() <= 1 {
        // A single row cannot repeat a gap — take the visual gaps as they are.
        1
    } else {
        // A real column gap repeats in many rows; a wide space inside a single
        // log line does not.
        std::cmp::max(2, (rows.len() as f32 * 0.3).ceil() as usize)
    };

    // Column gaps are wide compared to the text itself. The median word width
    // is the yardstick: a table column is separated by more space than one
    // character, while the word spacing inside a log line is not.
    let mut widths: Vec<i32> = words
        .iter()
        .filter(|w| !w.text.trim().is_empty())
        .map(|w| w.w.max(0))
        .collect();
    widths.sort_unstable();
    let median_width = if widths.is_empty() {
        0
    } else {
        widths[widths.len() / 2]
    };
    let width_floor = ((median_width as f32) * opts.gap_factor).ceil() as i32;

    let mut cuts: Vec<i32> = Vec::new();
    for pair in merged.windows(2) {
        let (gap_start, gap_end) = (pair[0].1, pair[1].0);
        if gap_end - gap_start < opts.min_gap_px.max(width_floor) {
            continue;
        }
        // Count the rows that have a word on both sides of this gap.
        let mut spanning = 0usize;
        for row in rows {
            let mut left = false;
            let mut right = false;
            for &i in row {
                let (a, b) = words[i].x_range();
                if b <= gap_start {
                    left = true;
                }
                if a >= gap_end {
                    right = true;
                }
            }
            if left && right {
                spanning += 1;
            }
        }
        if spanning >= min_rows {
            cuts.push((gap_start + gap_end) / 2);
        }
    }
    cuts
}

/// Build the row/column grid from a flat word list.
pub fn grid_from_words(words: &[WordBox], opts: &GridOptions) -> Grid {
    let rows_idx = group_rows(words);
    if rows_idx.is_empty() {
        return Grid::default();
    }
    let cuts = find_separators(&rows_idx, words, opts);

    // Column spans from the cut points.
    let mut columns: Vec<ColumnSpan> = Vec::new();
    let mut start = i32::MIN;
    for cut in &cuts {
        columns.push(ColumnSpan {
            x0: if start == i32::MIN { i32::MIN } else { start },
            x1: *cut,
        });
        start = *cut;
    }
    columns.push(ColumnSpan {
        x0: if start == i32::MIN { i32::MIN } else { start },
        x1: i32::MAX,
    });

    let mut rows: Vec<Vec<Cell>> = Vec::with_capacity(rows_idx.len());
    for row in &rows_idx {
        let mut cells: Vec<Cell> = vec![Cell::default(); columns.len()];
        let mut idx: Vec<usize> = row.clone();
        idx.sort_by_key(|&i| words[i].x);
        for &i in &idx {
            let w = &words[i];
            let center = w.x_center();
            let col = columns
                .iter()
                .position(|c| center >= c.x0 && center < c.x1)
                .unwrap_or(columns.len() - 1);
            let cell = &mut cells[col];
            if !cell.text.is_empty() {
                cell.text.push(' ');
            }
            cell.text.push_str(w.text.trim());
            cell.words.push(i);
        }
        rows.push(cells);
    }

    Grid { rows, columns }
}

/// Convenience wrapper with the default options.
pub fn grid(words: &[WordBox]) -> Grid {
    grid_from_words(words, &GridOptions::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(text: &str, x: i32, y: i32, width: i32) -> WordBox {
        WordBox {
            text: text.to_string(),
            x,
            y,
            w: width,
            h: 20,
            conf: Some(0.9),
        }
    }

    /// Three rows, four columns, wide gaps between the columns.
    fn table_words() -> Vec<WordBox> {
        let mut v = Vec::new();
        for (row, cells) in [
            ["ID", "Signal", "Value", "Unit"],
            ["0x01", "MotorSpeed", "1200", "rpm"],
            ["0x02", "MotorTorque", "340", "Nm"],
        ]
        .iter()
        .enumerate()
        {
            let y = (row as i32) * 24;
            let xs = [10, 120, 320, 420];
            for (i, text) in cells.iter().enumerate() {
                v.push(w(text, xs[i], y, (text.chars().count() as i32) * 8));
            }
        }
        v
    }

    #[test]
    fn keeps_table_columns() {
        let g = grid(&table_words());
        assert_eq!(g.column_count(), 4);
        assert_eq!(g.row_count(), 3);
        assert_eq!(g.column_values(0), vec!["ID", "0x01", "0x02"]);
        assert_eq!(g.column_values(1), vec!["Signal", "MotorSpeed", "MotorTorque"]);
        assert_eq!(g.column_values(2), vec!["Value", "1200", "340"]);
        assert_eq!(g.column_values(3), vec!["Unit", "rpm", "Nm"]);
    }

    #[test]
    fn joins_words_inside_one_cell() {
        let g = grid(&table_words());
        // A cell with two words keeps both, separated by one space.
        let words = vec![
            w("Hello", 10, 0, 50),
            w("World", 64, 0, 50),
            w("Second", 200, 0, 60),
        ];
        let g2 = grid(&words);
        assert_eq!(g2.column_values(0).first().map(String::as_str), Some("Hello World"));
        assert_eq!(g2.column_values(1).first().map(String::as_str), Some("Second"));
        assert_eq!(g.column_values(0).first().map(String::as_str), Some("ID"));
    }

    #[test]
    fn treats_a_log_line_without_gaps_as_one_column() {
        let mut v = Vec::new();
        for (row, line) in [
            "2026-11-10 12:00:01 INFO boot ok",
            "2026-11-10 12:00:02 WARN disk 87%",
        ]
        .iter()
        .enumerate()
        {
            let y = (row as i32) * 20;
            let mut x = 0;
            for token in line.split(' ') {
                v.push(w(token, x, y, (token.chars().count() as i32) * 7));
                x += (token.chars().count() as i32) * 7 + 8;
            }
        }
        let g = grid(&v);
        assert_eq!(g.column_count(), 1);
        assert_eq!(g.row_count(), 2);
        assert!(g.column_values(0)[0].starts_with("2026-11-10 12:00:01 INFO"));
    }

    #[test]
    fn separates_a_column_that_only_shows_up_later() {
        // First two rows have no third column; the gap only repeats in two
        // rows, which is the default threshold.
        let mut v = Vec::new();
        for (row, cells) in [["a", "b"], ["c", "d"], ["e", "f"]].iter().enumerate() {
            let y = (row as i32) * 24;
            v.push(w(cells[0], 10, y, 16));
            v.push(w(cells[1], 300, y, 16));
        }
        let g = grid(&v);
        assert_eq!(g.column_count(), 2);
        assert_eq!(g.column_values(1), vec!["b", "d", "f"]);
    }

    #[test]
    fn renders_tsv_and_aligned_text() {
        let g = grid(&table_words());
        let tsv = g.to_tsv();
        let first = tsv.lines().next().unwrap();
        assert_eq!(first, "ID\tSignal\tValue\tUnit");
        let aligned = g.to_aligned_text();
        // Columns keep their order and the row count.
        assert_eq!(aligned.lines().count(), 3);
        assert!(aligned.lines().next().unwrap().starts_with("ID"));
    }

    #[test]
    fn column_text_is_paste_ready() {
        let g = grid(&table_words());
        assert_eq!(g.column_text(2), "Value\n1200\n340");
        // Out of range → empty, no panic.
        assert_eq!(g.column_text(9), "");
        assert!(g.column_values(9).is_empty());
    }
}
