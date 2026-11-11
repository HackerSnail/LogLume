//! Column reconstruction against the simulated OCR corpus (spec §9, Phase 0.5).
//!
//! The corpus in `docs/ocr-samples/` holds, per sample, the rendered table
//! screenshot, the ideal detector boxes and the expected grid. Feeding the
//! boxes into `lumelog_core::ocr` must reproduce the grid exactly — this is
//! the check that runs before an OCR engine exists, so a column-regression
//! shows up without any model.
//!
//! Regenerate the fixtures with `python docs/ocr-samples/generate.py`.

use lumelog_core::ocr::{grid, WordBox};
use std::fs;
use std::path::{Path, PathBuf};

fn samples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("docs")
        .join("ocr-samples")
}

fn read_boxes(path: &Path) -> Vec<WordBox> {
    let raw = fs::read_to_string(path).expect("boxes file");
    raw.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let mut parts = line.split('\t');
            let text = parts.next().expect("text").to_string();
            let x: i32 = parts.next().expect("x").parse().expect("x number");
            let y: i32 = parts.next().expect("y").parse().expect("y number");
            let w: i32 = parts.next().expect("w").parse().expect("w number");
            let h: i32 = parts.next().expect("h").parse().expect("h number");
            WordBox {
                text,
                x,
                y,
                w,
                h,
                conf: Some(0.95),
            }
        })
        .collect()
}

fn read_truth(path: &Path) -> Vec<Vec<String>> {
    let raw = fs::read_to_string(path).expect("truth file");
    raw.lines()
        .filter(|l| !l.is_empty())
        .map(|line| line.split('\t').map(|c| c.to_string()).collect())
        .collect()
}

#[test]
fn corpus_reconstructs_from_ideal_boxes() {
    let dir = samples_dir();
    let mut names: Vec<String> = fs::read_dir(&dir)
        .expect("samples dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".boxes.tsv"))
        .map(|n| n.trim_end_matches(".boxes.tsv").to_string())
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no samples found in {}", dir.display());

    let mut failures: Vec<String> = Vec::new();
    let mut cells_checked = 0usize;
    for name in &names {
        let boxes = read_boxes(&dir.join(format!("{}.boxes.tsv", name)));
        let truth = read_truth(&dir.join(format!("{}.truth.tsv", name)));
        let got = grid(&boxes);

        if got.row_count() != truth.len() {
            failures.push(format!(
                "{}: rows {} != expected {}",
                name,
                got.row_count(),
                truth.len()
            ));
            continue;
        }
        if got.column_count() != truth[0].len() {
            failures.push(format!(
                "{}: columns {} != expected {}",
                name,
                got.column_count(),
                truth[0].len()
            ));
            continue;
        }
        for (r, (row, want_row)) in got.rows.iter().zip(truth.iter()).enumerate() {
            for (c, (cell, want)) in row.iter().zip(want_row.iter()).enumerate() {
                cells_checked += 1;
                if cell.text != *want {
                    failures.push(format!(
                        "{}: cell[{}][{}] = {:?} != {:?}",
                        name, r, c, cell.text, want
                    ));
                }
            }
        }
        // Column extraction must reproduce the column as newline separated text.
        for c in 0..truth[0].len() {
            let want: Vec<String> = truth.iter().map(|r| r[c].clone()).collect();
            let got_col = got.column_text(c);
            if got_col != want.join("\n") {
                failures.push(format!("{}: column {} extraction mismatch", name, c));
            }
        }
    }

    println!(
        "corpus: {} samples, {} cells reconstructed exactly",
        names.len(),
        cells_checked
    );
    assert!(
        failures.is_empty(),
        "corpus reconstruction mismatches:\n{}",
        failures.join("\n")
    );
}
