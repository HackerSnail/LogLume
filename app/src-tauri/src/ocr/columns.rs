//! Pixel level column detection (spec §10.2, Phase 1.5).
//!
//! The CTC peaks of the recognizer are too coarse to tell a letter spacing
//! from a word spacing, so the *columns* of a table screenshot are found in
//! the pixels instead: the vertical ink projection of a table has empty
//! strips between the columns, and those strips are exact.
//!
//! The strips are then cut out and recognized one by one — a narrow strip is
//! a proper text line for the recognizer, and every word it returns belongs
//! to exactly one column.

use crate::ocr::DecodedImage;

/// Tuning for the column detection.
#[derive(Debug, Clone, Copy)]
pub struct ColumnOptions {
    /// A gap must be at least this many pixels wide to count as a separator.
    pub min_gap_px: i32,
    /// A column gap may have ink on at most this fraction of the text rows.
    ///
    /// Zero is the safe default: a real column gap is empty in *every* row,
    /// while a sparse value whose glyphs shift from row to row ("00 00 00")
    /// leaves x positions empty in most rows — those must not look like a gap.
    pub max_ink_rows: f32,
    /// Minimum ink coverage of a row for it to count as a horizontal rule.
    pub rule_row_coverage: f32,
    /// Maximum height of a horizontal rule in pixels.
    pub rule_max_height: i32,
    /// Fraction of the text rows a gap must repeat in to count as a column gap.
    pub min_repeated_rows: f32,
    /// How far (pixels) the ink edges may be from the gap and still count as
    /// aligned.
    pub align_tolerance_px: i32,
}

impl Default for ColumnOptions {
    fn default() -> Self {
        ColumnOptions {
            // A column gap is much wider than the spacing inside a value
            // (e.g. the spaces between the bytes of "00 1A 2B"): 12px keeps
            // the two apart.
            min_gap_px: 12,
            max_ink_rows: 0.1,
            rule_row_coverage: 0.6,
            rule_max_height: 3,
            min_repeated_rows: 0.3,
            align_tolerance_px: 3,
        }
    }
}

/// One column of the table, in image pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColumnStrip {
    /// Left edge (inclusive).
    pub x0: i32,
    /// Right edge (exclusive).
    pub x1: i32,
}

impl ColumnStrip {
    /// Width in pixels.
    pub fn width(&self) -> i32 {
        self.x1 - self.x0
    }
}

/// Grayscale + Otsu threshold: `true` means ink.
pub fn binarize(image: &DecodedImage) -> Vec<bool> {
    let (w, h) = (image.width as usize, image.height as usize);
    let mut gray = vec![0u8; w * h];
    for (i, px) in image.bgra.chunks(4).enumerate() {
        // BGRA → luminance
        let g = (0.114 * px[0] as f32 + 0.587 * px[1] as f32 + 0.299 * px[2] as f32) as u32;
        gray[i] = g.min(255) as u8;
    }
    // Otsu: pick the threshold that maximizes the between class variance.
    let mut hist = [0u32; 256];
    for g in gray.iter() {
        hist[*g as usize] += 1;
    }
    let total = gray.len() as u64;
    let mut sum_all = 0u64;
    for (i, c) in hist.iter().enumerate() {
        sum_all += *c as u64 * i as u64;
    }
    let mut sum_b = 0u64;
    let mut w_b = 0u64;
    let mut best = (0f64, 0usize);
    for (t, count) in hist.iter().enumerate() {
        w_b += *count as u64;
        if w_b == 0 {
            continue;
        }
        let w_f = total - w_b;
        if w_f == 0 {
            break;
        }
        sum_b += *count as u64 * t as u64;
        let m_b = sum_b as f64 / w_b as f64;
        let m_f = (sum_all - sum_b) as f64 / w_f as f64;
        let between = w_b as f64 * w_f as f64 * (m_b - m_f) * (m_b - m_f);
        if between > best.0 {
            best = (between, t);
        }
    }
    let threshold = best.1 as u8;
    gray.iter().map(|g| *g <= threshold).collect()
}

/// Rows that are a horizontal rule (a table border) rather than text.
///
/// A rule is a thin band of ink spanning most of the width. A dark header
/// bar is thick, so it stays untouched.
fn rule_rows(ink: &[bool], w: usize, h: usize, opts: &ColumnOptions) -> Vec<bool> {
    let mut is_rule = vec![false; h];
    let min_ink = (opts.rule_row_coverage * w as f32) as usize;
    for y in 0..h {
        let count = ink[y * w..(y + 1) * w].iter().filter(|b| **b).count();
        if count < min_ink {
            continue;
        }
        // Thin? The band must not be taller than a couple of pixels.
        // A dense band around the row that is taller than a rule is a thick
        // header, not a rule — so look at the neighbours as well.
        let mut above = 0usize;
        while above < y && row_is_dense(ink, w, h, y - above - 1, min_ink) {
            above += 1;
        }
        let mut below = 0usize;
        loop {
            let next = y + below + 1;
            if next >= h || !row_is_dense(ink, w, h, next, min_ink) {
                break;
            }
            below += 1;
        }
        if above + below < opts.rule_max_height as usize {
            is_rule[y] = true;
        }
    }
    is_rule
}

fn row_is_dense(ink: &[bool], w: usize, h: usize, y: usize, min_ink: usize) -> bool {
    if y >= h {
        return false;
    }
    ink[y * w..(y + 1) * w].iter().filter(|b| **b).count() >= min_ink
}

/// Find the table columns by their pixel level gaps.
///
/// Returns the strips that contain ink; an image without table structure
/// yields a single strip covering the whole width.
pub fn column_strips(image: &DecodedImage, opts: &ColumnOptions) -> Vec<ColumnStrip> {
    let (w, h) = (image.width as usize, image.height as usize);
    if w < 8 || h < 8 {
        return vec![ColumnStrip {
            x0: 0,
            x1: image.width as i32,
        }];
    }
    let mut ink = binarize(image);
    let rules = rule_rows(&ink, w, h, opts);
    // Rules are not text: drop them so they cannot mask the column gaps.
    for (y, is_rule) in rules.iter().enumerate() {
        if *is_rule {
            for x in 0..w {
                ink[y * w + x] = false;
            }
        }
    }

    // The ink segments of every row: the left and right edges of a column
    // line up across the rows, the edges of a value do not.
    let mut segments: Vec<Vec<(usize, usize)>> = Vec::with_capacity(h);
    for y in 0..h {
        let row = &ink[y * w..(y + 1) * w];
        let mut segs: Vec<(usize, usize)> = Vec::new();
        let mut x = 0usize;
        while x < w {
            if row[x] {
                let start = x;
                while x < w && row[x] {
                    x += 1;
                }
                segs.push((start, x));
            } else {
                x += 1;
            }
        }
        segments.push(segs);
    }

    // Per column: how many rows carry an ink *segment* there.
    //
    // Counting pixels would be wrong: a narrow glyph ("0") leaves wide empty
    // stretches inside a value, and those would look like a column gap.
    // Counting segments — one per row, however wide the segment is — keeps
    // sparse content dense and leaves only the real gaps empty.
    let mut rows_with_ink = vec![0u32; w];
    let mut text_rows = 0u32;
    for y in 0..h {
        let row = &ink[y * w..(y + 1) * w];
        let mut any = false;
        let mut x = 0usize;
        while x < w {
            if row[x] {
                let start = x;
                while x < w && row[x] {
                    x += 1;
                }
                for slot in rows_with_ink.iter_mut().take(x).skip(start) {
                    *slot += 1;
                }
                any = true;
            } else {
                x += 1;
            }
        }
        if any {
            text_rows += 1;
        }
    }
    if text_rows == 0 {
        return vec![ColumnStrip {
            x0: 0,
            x1: image.width as i32,
        }];
    }
    let allowed = ((text_rows as f32) * opts.max_ink_rows).ceil() as u32;

    // Maximal x ranges with (almost) no ink at all.
    let mut gaps: Vec<(i32, i32)> = Vec::new();
    let mut start: Option<i32> = None;
    for (x, count) in rows_with_ink.iter().enumerate() {
        if *count <= allowed {
            if start.is_none() {
                start = Some(x as i32);
            }
        } else if let Some(s) = start.take() {
            gaps.push((s, x as i32));
        }
    }
    if let Some(s) = start {
        gaps.push((s, w as i32));
    }
    gaps.retain(|(a, b)| b - a >= opts.min_gap_px.min(4));

    // A column gap is a place where the ink *lines up*: the last block on the
    // left ends at the same x in every row, and the first block on the right
    // starts at the same x in every row. Inside a value the blocks shift from
    // row to row ("00 1A 2B" vs "00 00 00 64"), so the same gap does not
    // repeat — that is what tells a column from a wide space.
    // At least two rows have to agree: a single row can align by accident.
    let min_rows =
        (((text_rows as f32) * opts.min_repeated_rows).ceil() as u32).max(2);
    let tol = opts.align_tolerance_px.max(1);
    let mut separators: Vec<(i32, i32)> = Vec::new();
    for (a, b) in gaps {
        if b - a < opts.min_gap_px {
            continue;
        }
        let mut aligned = 0u32;
        for segs in &segments {
            // The first block right of the gap starts at the same x in every
            // row — that is the column's left edge. (The right edge of the
            // previous column is *not* usable: the ink of a glyph like "00"
            // ends a few pixels further right than the ink of "64".)
            let right = segs.iter().find(|(s, _)| *s >= b as usize);
            let Some((rs, _)) = right else { continue };
            if (*rs as i32 - b).abs() <= tol {
                aligned += 1;
            }
        }
        if aligned >= min_rows {
            separators.push((a, b));
        }
    }

    // The strips are what lies between the separators.
    let mut strips: Vec<ColumnStrip> = Vec::new();
    let mut cursor = 0i32;
    for (a, b) in separators {
        if a > cursor {
            strips.push(ColumnStrip {
                x0: cursor,
                x1: a,
            });
        }
        cursor = b;
    }
    if cursor < w as i32 {
        strips.push(ColumnStrip {
            x0: cursor,
            x1: w as i32,
        });
    }
    // Keep the strips that actually carry text.
    let strips: Vec<ColumnStrip> = strips
        .into_iter()
        .filter(|s| {
            let (x0, x1) = (s.x0 as usize, s.x1 as usize);
            (0..h).any(|y| ink[y * w + x0..y * w + x1].iter().any(|b| *b))
        })
        .collect();
    if strips.is_empty() {
        vec![ColumnStrip {
            x0: 0,
            x1: image.width as i32,
        }]
    } else {
        strips
    }
}

/// Paint out the horizontal rules of a table.
///
/// The detector reads the rules as text and misses whole cells because of
/// them, so they are removed before the image goes to the models.
pub fn strip_rules(image: &DecodedImage, opts: &ColumnOptions) -> DecodedImage {
    let (w, h) = (image.width as usize, image.height as usize);
    if w < 8 || h < 8 {
        return DecodedImage {
            width: image.width,
            height: image.height,
            bgra: image.bgra.clone(),
        };
    }
    let ink = binarize(image);
    let rules = rule_rows(&ink, w, h, opts);
    if !rules.iter().any(|r| *r) {
        return DecodedImage {
            width: image.width,
            height: image.height,
            bgra: image.bgra.clone(),
        };
    }
    let mut bgra = image.bgra.clone();
    for (y, is_rule) in rules.iter().enumerate() {
        if !*is_rule {
            continue;
        }
        for x in 0..w {
            let i = (y * w + x) * 4;
            bgra[i] = 255;
            bgra[i + 1] = 255;
            bgra[i + 2] = 255;
        }
    }
    DecodedImage {
        width: image.width,
        height: image.height,
        bgra,
    }
}

/// Crop the image to a column strip (the pixels stay BGRA).
pub fn crop_strip(image: &DecodedImage, strip: &ColumnStrip) -> DecodedImage {
    let (w, h) = (image.width as usize, image.height as usize);
    let x0 = strip.x0.max(0) as usize;
    let x1 = (strip.x1.max(0) as usize).min(w);
    let width = x1.saturating_sub(x0).max(1);
    let mut bgra = Vec::with_capacity(width * h * 4);
    for y in 0..h {
        let start = (y * w + x0) * 4;
        bgra.extend_from_slice(&image.bgra[start..start + width * 4]);
    }
    DecodedImage {
        width: width as u32,
        height: h as u32,
        bgra,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 200×80 image with two text columns separated by a 30px gap and a
    /// horizontal rule across the whole width.
    fn table_image() -> DecodedImage {
        let (w, h) = (200usize, 80usize);
        let mut bgra = vec![255u8; w * h * 4];
        let mut put = |x: usize, y: usize| {
            let i = (y * w + x) * 4;
            bgra[i] = 0;
            bgra[i + 1] = 0;
            bgra[i + 2] = 0;
        };
        // left column text at x=10..40, right column at x=100..140
        for y in 5..15 {
            for x in 10..40 {
                put(x, y);
            }
        }
        for y in 5..15 {
            for x in 100..140 {
                put(x, y);
            }
        }
        // a table rule across the whole width
        for x in 0..w {
            put(x, 40);
        }
        DecodedImage {
            width: w as u32,
            height: h as u32,
            bgra,
        }
    }

    #[test]
    fn finds_the_gap_between_two_columns() {
        let strips = column_strips(&table_image(), &ColumnOptions::default());
        assert_eq!(strips.len(), 2, "strips: {:?}", strips);
        assert!(strips[0].x1 <= 50, "left column ends before the gap");
        assert!(strips[1].x0 >= 90, "right column starts after the gap");
    }

    #[test]
    fn a_horizontal_rule_does_not_hide_the_columns() {
        // The rule covers every column; without removing it the projection
        // would never reach zero and no gap would be found.
        let strips = column_strips(&table_image(), &ColumnOptions::default());
        assert!(strips.len() >= 2);
    }

    #[test]
    fn plain_text_yields_one_strip() {
        let (w, h) = (120usize, 40usize);
        let mut bgra = vec![255u8; w * h * 4];
        for y in 5..15 {
            for x in 5..60 {
                let i = (y * w + x) * 4;
                bgra[i] = 0;
                bgra[i + 1] = 0;
                bgra[i + 2] = 0;
            }
        }
        let image = DecodedImage {
            width: w as u32,
            height: h as u32,
            bgra,
        };
        let strips = column_strips(&image, &ColumnOptions::default());
        assert_eq!(strips.len(), 1);
    }

    /// A table whose values are wide and shift from row to row: the gap
    /// between the columns is aligned, the gaps inside the values are not.
    #[test]
    fn alignment_beats_width() {
        let (w, h) = (240usize, 90usize);
        let mut bgra = vec![255u8; w * h * 4];
        let mut put = |x: usize, y: usize| {
            let i = (y * w + x) * 4;
            bgra[i] = 0;
            bgra[i + 1] = 0;
            bgra[i + 2] = 0;
        };
        // Two columns: the left one at x=10, the right one at x=140.
        for row in 0..3 {
            let y = 10 + row * 25;
            // left cell: a short value
            for x in 10..40 {
                put(x, y);
            }
            // right cell: a value whose internal gap moves from row to row
            let gap = 150 + row * 6;
            for x in 140..gap {
                put(x, y);
            }
            for x in (gap + 14)..(gap + 40) {
                put(x, y);
            }
        }
        let image = DecodedImage {
            width: w as u32,
            height: h as u32,
            bgra,
        };
        let strips = column_strips(&image, &ColumnOptions::default());
        assert_eq!(strips.len(), 2, "strips: {:?}", strips);
        // The column gap is at x≈140, the shifting inner gap must not split it.
        assert!(strips[1].x0 >= 130 && strips[1].x0 <= 150, "right column starts at {}", strips[1].x0);
    }

    #[test]
    fn crop_keeps_the_rows_and_cuts_the_columns() {
        let image = table_image();
        let strip = ColumnStrip { x0: 100, x1: 140 };
        let cropped = crop_strip(&image, &strip);
        assert_eq!(cropped.width, 40);
        assert_eq!(cropped.height, 80);
        assert_eq!(cropped.bgra.len(), 40 * 80 * 4);
    }
}
