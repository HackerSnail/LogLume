//! OCR grid reconstruction benchmark (image → words → rows/columns).
//!
//! The OCR engines differ, but the column reconstruction is the same work for
//! every engine, so this bench is the perf baseline for that step: a table
//! screenshot of `ROWS × COLS` cells must be turned into a grid fast enough
//! to stay invisible next to the recognition itself.
//!
//! Manual run: `cargo bench -p lumelog-core --bench ocr_grid`

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lumelog_core::ocr::{grid_from_words, GridOptions, WordBox};

const COLS: usize = 8;
const ROWS: usize = 400;

/// A table screenshot worth of words: 8 columns, 400 rows, 3 words per cell.
fn table_words() -> Vec<WordBox> {
    let mut words = Vec::with_capacity(ROWS * COLS * 3);
    let col_stride = 180;
    let row_stride = 24;
    for r in 0..ROWS {
        let y = (r as i32) * row_stride;
        for c in 0..COLS {
            let x = 20 + (c as i32) * col_stride;
            for (k, text) in ["Signal", "value", "unit"].iter().enumerate() {
                let w = (text.chars().count() as i32) * 7;
                words.push(WordBox {
                    text: (*text).to_string(),
                    x: x + (k as i32) * 40,
                    y,
                    w,
                    h: 18,
                    conf: 0.95,
                });
            }
        }
    }
    words
}

fn bench_grid(c: &mut Criterion) {
    let words = table_words();
    let opts = GridOptions::default();
    c.bench_function("ocr_grid_from_words_400x8", |b| {
        b.iter(|| {
            let g = grid_from_words(black_box(&words), &opts);
            black_box(g.column_count())
        })
    });
}

criterion_group!(benches, bench_grid);
criterion_main!(benches);
