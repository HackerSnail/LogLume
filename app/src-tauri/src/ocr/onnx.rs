//! PP-OCR via ONNX Runtime from the offline component package (spec §8).
//!
//! The native ONNX Runtime library lives in the component and is loaded at
//! runtime with `ort::init_from` — the main binary never links it, so the
//! installer stays small (P6). The pipeline is the classic PP-OCR two stages:
//!
//! ```text
//! image → det (DB) → text boxes → rec (CTC) → text per box → word boxes
//! ```
//!
//! No direction classifier (the input is always upright) and no separate
//! English model (the `ch` model covers Chinese, English and digits).

use crate::ocr::columns::{column_strips, crop_strip, ColumnOptions};
use crate::ocr::component;
use crate::ocr::{DecodedImage, LanguageChoice, OcrError, Progress};
use lumelog_core::ocr::{ColumnSpan, WordBox};
use std::path::Path;
use std::sync::OnceLock;

/// Detection thresholds (PP-OCR defaults).
const DET_THRESHOLD: f32 = 0.3;
const DET_UNCLIP_RATIO: f32 = 1.5;
/// Recognition input height; the width follows the box aspect ratio.
const REC_HEIGHT: usize = 48;
const REC_MAX_WIDTH: usize = 320;
/// The recognizer's maximum width/height ratio (320 / 48). A wider line would
/// be squeezed into 320 px and the CTC time steps would no longer resolve
/// single characters — so long lines are split into chunks of this ratio.
const REC_MAX_ASPECT: f32 = REC_MAX_WIDTH as f32 / REC_HEIGHT as f32;
/// Overlap between two chunks, so a word cut at a seam still appears whole in
/// the next chunk (the duplicated characters are dropped afterwards).
const CHUNK_OVERLAP_PX: i32 = 24;

/// One detected text box in image pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextBox {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// Detection confidence (mean probability inside the box).
    pub score: f32,
}

/// Run the PP-OCR pipeline over an image.
///
/// Returns the words and — in table mode — the column spans the caller should
/// use instead of the gap based column detection.
/// Words, the column spans when the engine found the table structure, and the
/// language tag.
pub type OcrWords = (Vec<WordBox>, Option<Vec<ColumnSpan>>, String);

pub fn recognize(
    image: &DecodedImage,
    _language: LanguageChoice,
    progress: Progress<'_>,
) -> Result<OcrWords, OcrError> {
    progress("查找 OCR 组件包", 10);
    let comp = component::locate(component::COMPONENT_VERSION).map_err(|e| {
        OcrError::new("component_missing", format!("{}", e))
    })?;

    progress("加载 ONNX Runtime", 20);
    let runtime = comp.runtime_path();
    let env = environment(&runtime)?;

    let det = comp
        .det_model_path()
        .ok_or_else(|| OcrError::new("component_invalid", "组件包缺少检测模型"))?;
    let rec = comp
        .rec_model_path()
        .ok_or_else(|| OcrError::new("component_invalid", "组件包缺少识别模型"))?;
    let dict = comp
        .manifest
        .dict
        .as_ref()
        .map(|d| comp.root.join(&d.path))
        .ok_or_else(|| OcrError::new("component_invalid", "组件包缺少字符字典"))?;
    let chars = read_dict(&dict)?;

    // Table mode: the columns are found in the pixels (exact) and every
    // column is recognized on its own, so a word can never be split across
    // two columns. Without table structure the whole image is one strip.
    progress("检测表格列", 30);
    let strips = column_strips(image, &ColumnOptions::default());

    if strips.len() >= 2 {
        // Each column is a narrow image: the recognizer gets a proper text
        // line per cell, and the whole line text stays intact.
        let mut words: Vec<WordBox> = Vec::new();
        let mut bounds: Vec<ColumnSpan> = Vec::new();
        for strip in &strips {
            progress("检测文本区域", 35);
            let piece = crop_strip(image, strip);
            let boxes = detect(env, &det, &piece)?;
            progress("识别文字", 60);
            for (text, y, h) in recognize_lines(env, &rec, &chars, &piece, &boxes)? {
                words.push(WordBox {
                    text,
                    x: strip.x0,
                    y,
                    w: strip.width(),
                    h,
                    conf: None,
                });
            }
            bounds.push(ColumnSpan {
                x0: strip.x0,
                x1: strip.x1,
            });
        }
        return Ok((words, Some(bounds), "onnx".to_string()));
    }

    let boxes = detect(env, &det, image)?;
    let words = recognize_boxes(env, &rec, &chars, image, &boxes)?;
    Ok((words, None, "onnx".to_string()))
}

/// The ONNX Runtime environment, created once per process.
///
/// ONNX Runtime allows a single environment per process, so a second
/// recognition run must reuse the first one instead of loading again.
fn environment(runtime: &Path) -> Result<&'static ort::environment::Environment, OcrError> {
    static ENV: OnceLock<Result<ort::environment::Environment, String>> = OnceLock::new();
    let env = ENV.get_or_init(|| {
        ort::init_from(runtime)
            .map_err(|e| format!("加载 ONNX Runtime 失败：{}", e))?
            .with_name("lumelog")
            .build()
            .map_err(|e| format!("初始化推理环境失败：{}", e))
    });
    match env {
        Ok(env) => Ok(env),
        Err(msg) => Err(OcrError::new("component_invalid", msg.clone())),
    }
}

/// Read the character dictionary (one character per line, blank is implicit).
fn read_dict(path: &std::path::Path) -> Result<Vec<char>, OcrError> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| OcrError::new("component_invalid", format!("读取字符字典失败：{}", e)))?;
    Ok(raw.lines().map(|l| l.chars().next().unwrap_or(' ')).collect())
}

/// Detection stage: run the model and turn the probability map into boxes.
fn detect(
    env: &ort::environment::Environment,
    model: &std::path::Path,
    image: &DecodedImage,
) -> Result<Vec<TextBox>, OcrError> {
    let mut builder = ort::session::Session::builder(env)
        .map_err(|e| OcrError::new("component_invalid", format!("创建推理会话失败：{}", e)))?;
    let mut session = builder
        .commit_from_file(model)
        .map_err(|e| OcrError::new("component_invalid", format!("加载检测模型失败：{}", e)))?;

    let (data, nw, nh, scale) = preprocess_det(image);
    let tensor = ort::value::Tensor::from_array((vec![1usize, 3, nh, nw], data))
        .map_err(|e| OcrError::engine(format!("构造检测输入失败：{}", e)))?;
    let outputs = session
        .run(ort::inputs!["x" => tensor])
        .map_err(|e| OcrError::engine(format!("检测推理失败：{}", e)))?;

    let (shape, prob) = outputs[0]
        .try_extract_tensor::<f32>()
        .map_err(|e| OcrError::engine(format!("读取检测输出失败：{}", e)))?;
    let (h, w) = (shape[shape.len() - 2] as usize, shape[shape.len() - 1] as usize);
    let boxes = boxes_from_prob(prob, w, h, DET_THRESHOLD, scale);
    Ok(boxes)
}

/// Scale the image for detection: longest side to 960 (PP-OCR default),
/// rounded to a multiple of 32. Returns (pixels, width, height, scale).
fn preprocess_det(image: &DecodedImage) -> (Vec<f32>, usize, usize, f32) {
    let (w, h) = (image.width as usize, image.height as usize);
    let target = 960usize;
    let scale = if w.max(h) > target {
        target as f32 / w.max(h) as f32
    } else {
        1.0
    };
    let nw = (((w as f32 * scale) / 32.0).round() as usize).max(32) * 32;
    let nh = (((h as f32 * scale) / 32.0).round() as usize).max(32) * 32;
    let (rw, rh) = (nw.min(w.max(1)), nh.min(h.max(1)));
    let mut data = vec![0f32; 3 * nw * nh];
    for y in 0..rh {
        for x in 0..rw {
            let src = (y * w + x) * 4;
            let p = &image.bgra[src..src + 4];
            let (b, g, r) = (p[0] as f32, p[1] as f32, p[2] as f32);
            let idx = y * nw + x;
            data[idx] = r / 255.0;
            data[nw * nh + idx] = g / 255.0;
            data[2 * nw * nh + idx] = b / 255.0;
        }
    }
    (data, nw, nh, scale)
}

/// Turn the probability map into text boxes.
///
/// Connected components of the thresholded map give the boxes; a component
/// smaller than the minimum size is noise (spec §9.3's noise-box case).
fn boxes_from_prob(
    prob: &[f32],
    w: usize,
    h: usize,
    threshold: f32,
    scale: f32,
) -> Vec<TextBox> {
    let mut mask = vec![false; w * h];
    for (i, p) in prob.iter().enumerate().take(w * h) {
        mask[i] = *p >= threshold;
    }
    let mut visited = vec![false; w * h];
    let mut boxes: Vec<TextBox> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for start in 0..w * h {
        if !mask[start] || visited[start] {
            continue;
        }
        stack.clear();
        stack.push(start);
        visited[start] = true;
        let mut min_x = w;
        let mut max_x = 0usize;
        let mut min_y = h;
        let mut max_y = 0usize;
        let mut count = 0usize;
        let mut sum = 0f32;
        while let Some(idx) = stack.pop() {
            let x = idx % w;
            let y = idx / w;
            count += 1;
            sum += prob[idx];
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
            for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                let n = ny as usize * w + nx as usize;
                if mask[n] && !visited[n] {
                    visited[n] = true;
                    stack.push(n);
                }
            }
        }
        let bw = max_x - min_x + 1;
        let bh = max_y - min_y + 1;
        if count < 5 || bw < 3 || bh < 3 {
            continue;
        }
        // Unclip: grow the box a bit, then scale back to image pixels.
        let grow_x = ((bw as f32) * (DET_UNCLIP_RATIO - 1.0) / 2.0).round() as i32;
        let grow_y = ((bh as f32) * (DET_UNCLIP_RATIO - 1.0) / 2.0).round() as i32;
        let inv = if scale > 0.0 { 1.0 / scale } else { 1.0 };
        boxes.push(TextBox {
            x: ((min_x as i32 - grow_x).max(0) as f32 * inv).round() as i32,
            y: ((min_y as i32 - grow_y).max(0) as f32 * inv).round() as i32,
            w: ((bw as i32 + 2 * grow_x) as f32 * inv).round() as i32,
            h: ((bh as i32 + 2 * grow_y) as f32 * inv).round() as i32,
            score: sum / count as f32,
        });
    }
    boxes.sort_by_key(|b| (b.y, b.x));
    boxes
}

/// Recognition stage for table mode: one text line per detected box.
///
/// The whole line text is returned as-is (no character level positioning) —
/// the column is already known from the pixels, so the recognizer only has to
/// read the text.
fn recognize_lines(
    env: &ort::environment::Environment,
    model: &std::path::Path,
    chars: &[char],
    image: &DecodedImage,
    boxes: &[TextBox],
) -> Result<Vec<(String, i32, i32)>, OcrError> {
    let mut builder = ort::session::Session::builder(env)
        .map_err(|e| OcrError::new("component_invalid", format!("创建推理会话失败：{}", e)))?;
    let mut session = builder
        .commit_from_file(model)
        .map_err(|e| OcrError::new("component_invalid", format!("加载识别模型失败：{}", e)))?;
    let mut lines = Vec::new();
    for b in boxes {
        // A cell wider than the recognizer's aspect ratio is read in chunks,
        // otherwise the text would be squeezed and characters would be lost.
        let max_w = ((b.h as f32) * REC_MAX_ASPECT).round() as i32;
        if max_w < 16 {
            continue;
        }
        let step = (max_w - CHUNK_OVERLAP_PX).max(16);
        let mut chunk_x = b.x;
        let mut text_all = String::new();
        let mut chunk_index = 0usize;
        // Characters covered by the previous chunk's overlap would be read
        // twice; drop them from every chunk but the first.
        let overlap_chars = (CHUNK_OVERLAP_PX as f32 / (b.h as f32 * 0.55)).round() as usize;
        while chunk_x < b.x + b.w {
            let chunk_w = max_w.min(b.x + b.w - chunk_x);
            if chunk_w <= 0 {
                break;
            }
            let chunk = TextBox {
                x: chunk_x,
                y: b.y,
                w: chunk_w,
                h: b.h,
                score: b.score,
            };
            let (crop, rw) = crop_resize(image, &chunk);
            let tensor = ort::value::Tensor::from_array((vec![1usize, 3, REC_HEIGHT, rw], crop))
                .map_err(|e| OcrError::engine(format!("构造识别输入失败：{}", e)))?;
            let outputs = session
                .run(ort::inputs!["x" => tensor])
                .map_err(|e| OcrError::engine(format!("识别推理失败：{}", e)))?;
            let (shape, data) = outputs[0]
                .try_extract_tensor::<f32>()
                .map_err(|e| OcrError::engine(format!("读取识别输出失败：{}", e)))?;
            let steps = shape[shape.len() - 2] as usize;
            let classes = shape[shape.len() - 1] as usize;
            let (text, _conf, _peaks) = ctc_decode_with_peaks(data, steps, classes, chars);
            let trimmed = text.trim();
            let text: String = if chunk_index == 0 {
                trimmed.to_string()
            } else {
                trimmed.chars().skip(overlap_chars).collect::<String>()
            };
            text_all.push_str(&text);
            chunk_index += 1;
            chunk_x += step;
        }
        let text = text_all.trim().to_string();
        if text.is_empty() {
            continue;
        }
        lines.push((text, b.y, b.h));
    }
    Ok(lines)
}

/// Recognition stage: crop each box, run the model, decode the CTC output.
fn recognize_boxes(
    env: &ort::environment::Environment,
    model: &std::path::Path,
    chars: &[char],
    image: &DecodedImage,
    boxes: &[TextBox],
) -> Result<Vec<WordBox>, OcrError> {
    let mut builder = ort::session::Session::builder(env)
        .map_err(|e| OcrError::new("component_invalid", format!("创建推理会话失败：{}", e)))?;
    let mut session = builder
        .commit_from_file(model)
        .map_err(|e| OcrError::new("component_invalid", format!("加载识别模型失败：{}", e)))?;

    let mut words: Vec<WordBox> = Vec::new();
    for b in boxes {
        // A table row is one very wide "line" for the detector; the recognizer
        // can only take 48×320, so the line is cut into chunks of the model's
        // aspect ratio and every chunk is recognized on its own.
        let max_w = ((b.h as f32) * REC_MAX_ASPECT).round() as i32;
        if max_w < 16 {
            continue;
        }
        let step = (max_w - CHUNK_OVERLAP_PX).max(16);
        let mut chunk_x = b.x;
        let mut covered_to = b.x;
        while chunk_x < b.x + b.w {
            let chunk_w = max_w.min(b.x + b.w - chunk_x);
            if chunk_w <= 0 {
                break;
            }
            let chunk = TextBox {
                x: chunk_x,
                y: b.y,
                w: chunk_w,
                h: b.h,
                score: b.score,
            };
            let (crop, rw) = crop_resize(image, &chunk);
            let tensor = ort::value::Tensor::from_array((vec![1usize, 3, REC_HEIGHT, rw], crop))
                .map_err(|e| OcrError::engine(format!("构造识别输入失败：{}", e)))?;
            let outputs = session
                .run(ort::inputs!["x" => tensor])
                .map_err(|e| OcrError::engine(format!("识别推理失败：{}", e)))?;
            let (shape, data) = outputs[0]
                .try_extract_tensor::<f32>()
                .map_err(|e| OcrError::engine(format!("读取识别输出失败：{}", e)))?;
            let steps = shape[shape.len() - 2] as usize;
            let classes = shape[shape.len() - 1] as usize;
            let (text, _conf, peaks) = ctc_decode_with_peaks(data, steps, classes, chars);
            if text.trim().is_empty() {
                chunk_x += step;
                continue;
            }
            // The CTC peak of a character is where its ink was; the box is
            // placed there with an estimated glyph width. Word gaps and
            // column gaps then show up as real gaps between the boxes, which
            // is what the column reconstruction needs.
            let px_per_step = chunk_w as f32 / steps as f32;
            for (i, peak) in peaks.iter().enumerate() {
                let cx = (peak.step as f32 + 0.5) * px_per_step;
                // A glyph is never wider than the room to the next one: taking
                // the smaller of the estimate and the actual peak distance
                // keeps adjacent letters touching while a real word gap stays
                // a gap.
                let estimate = char_width(peak.ch, b.h).max(1);
                let w = match peaks.get(i + 1) {
                    Some(next) => {
                        let distance =
                            ((next.step - peak.step) as f32 * px_per_step).round() as i32;
                        estimate.min(distance.max(1))
                    }
                    None => estimate,
                };
                let x0 = chunk_x as f32 + cx - w as f32 / 2.0;
                let x1 = x0 + w as f32;
                // Drop what the previous chunk already covered (the overlap).
                if x1.round() as i32 <= covered_to {
                    continue;
                }
                words.push(WordBox {
                    text: peak.ch.to_string(),
                    x: x0.round() as i32,
                    y: b.y,
                    w,
                    h: b.h,
                    conf: Some(peak.conf),
                });
            }
            covered_to = chunk_x + chunk_w;
            chunk_x += step;
        }
    }
    Ok(words)
}

/// Crop a box and resize it to the recognition input size (1 × 3 × 48 × W).
/// Returns the pixels and the resized width.
fn crop_resize(image: &DecodedImage, b: &TextBox) -> (Vec<f32>, usize) {
    let (iw, ih) = (image.width as usize, image.height as usize);
    let x0 = b.x.max(0) as usize;
    let y0 = b.y.max(0) as usize;
    let x1 = ((b.x + b.w).max(0) as usize).min(iw);
    let y1 = ((b.y + b.h).max(0) as usize).min(ih);
    let cw = x1.saturating_sub(x0).max(1);
    let ch = y1.saturating_sub(y0).max(1);
    let ratio = cw as f32 / ch as f32;
    let rw = ((REC_HEIGHT as f32 * ratio) as usize).clamp(16, REC_MAX_WIDTH);
    let mut data = vec![0f32; 3 * REC_HEIGHT * rw];
    for y in 0..REC_HEIGHT {
        let sy = (y0 + (y * ch) / REC_HEIGHT).min(ih.saturating_sub(1));
        for x in 0..rw {
            let sx = (x0 + (x * cw) / rw).min(iw.saturating_sub(1));
            let src = (sy * iw + sx) * 4;
            let p = &image.bgra[src..src + 4];
            let idx = y * rw + x;
            data[idx] = p[2] as f32 / 255.0;
            data[REC_HEIGHT * rw + idx] = p[1] as f32 / 255.0;
            data[2 * REC_HEIGHT * rw + idx] = p[0] as f32 / 255.0;
        }
    }
    (data, rw)
}

/// One decoded character and the CTC time step it peaked at.
///
/// PP-OCR's recognizer emits characters with blank steps in between, so the
/// blanks are not word separators — but the *position* of the peak inside the
/// recognition crop is meaningful: it is where the character's ink was. That
/// is what lets a text line become word boxes (and therefore columns).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharPeak {
    pub ch: char,
    /// Time step where the character peaked.
    pub step: usize,
    /// Confidence at that step.
    pub conf: f32,
}

/// Greedy CTC decode that also reports where each character peaked.
pub fn ctc_decode_with_peaks(
    logits: &[f32],
    steps: usize,
    classes: usize,
    chars: &[char],
) -> (String, f32, Vec<CharPeak>) {
    let blank = 0usize;
    let mut text = String::new();
    let mut peaks: Vec<CharPeak> = Vec::new();
    let mut conf_sum = 0f32;
    let mut conf_count = 0usize;
    let mut prev = usize::MAX;
    let mut peak_conf = 0f32;

    for s in 0..steps {
        let row = &logits[s * classes..(s + 1) * classes];
        let mut best = 0usize;
        let mut best_v = f32::NEG_INFINITY;
        for (i, v) in row.iter().enumerate() {
            if *v > best_v {
                best_v = *v;
                best = i;
            }
        }
        if best != prev {
            if best != blank {
                if let Some(c) = chars.get(best - 1) {
                    peaks.push(CharPeak {
                        ch: *c,
                        step: s,
                        conf: best_v,
                    });
                    text.push(*c);
                    conf_sum += best_v;
                    conf_count += 1;
                }
            }
            prev = best;
            peak_conf = best_v;
        } else if best != blank && best_v > peak_conf {
            peak_conf = best_v;
        }
    }
    let conf = if conf_count > 0 {
        (conf_sum / conf_count as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (text, conf, peaks)
}

/// Estimated ink width of a character, in pixels, given the line height.
///
/// CJK glyphs are square (as wide as the line is tall); latin glyphs are about
/// half that. The estimate is what makes a word gap or a column gap visible
/// between two character boxes.
fn char_width(ch: char, line_height: i32) -> i32 {
    let wide = matches!(ch as u32,
        0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6);
    let factor = if wide { 1.0 } else { 0.55 };
    ((line_height as f32) * factor).round() as i32
}

/// Greedy CTC decode: per step take the best class, collapse repeats, drop
/// blanks. Returns the text and the mean confidence of the kept steps.
pub fn ctc_decode(
    logits: &[f32],
    steps: usize,
    classes: usize,
    chars: &[char],
) -> (String, f32) {
    let blank = 0usize;
    let mut text = String::new();
    let mut prev = usize::MAX;
    let mut conf_sum = 0f32;
    let mut conf_count = 0usize;
    for s in 0..steps {
        let row = &logits[s * classes..(s + 1) * classes];
        let mut best = 0usize;
        let mut best_v = f32::NEG_INFINITY;
        for (i, v) in row.iter().enumerate() {
            if *v > best_v {
                best_v = *v;
                best = i;
            }
        }
        if best != blank && best != prev {
            if let Some(c) = chars.get(best - 1) {
                text.push(*c);
                conf_sum += best_v;
                conf_count += 1;
            }
        }
        prev = best;
    }
    let conf = if conf_count > 0 {
        (conf_sum / conf_count as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (text, conf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctc_decodes_and_collapses_repeats() {
        // 3 classes: blank, 'a', 'b'; 4 steps.
        let chars = vec!['a', 'b'];
        let logits = vec![
            0.9, 0.1, 0.0, // blank
            0.1, 0.8, 0.1, // a
            0.1, 0.8, 0.1, // a (repeat → collapse)
            0.1, 0.1, 0.9, // b
        ];
        let (text, conf) = ctc_decode(&logits, 4, 3, &chars);
        assert_eq!(text, "ab");
        assert!(conf > 0.0 && conf <= 1.0);
    }

    #[test]
    fn ctc_ignores_blanks() {
        let chars = vec!['x'];
        let logits = vec![0.9, 0.1, 0.9, 0.1, 0.9, 0.1];
        let (text, _) = ctc_decode(&logits, 3, 2, &chars);
        assert_eq!(text, "");
    }

    #[test]
    fn probability_map_becomes_boxes() {
        // 8×8 map with one bright 3×3 blob at (2..5, 2..5).
        let mut prob = vec![0f32; 64];
        for y in 2..5 {
            for x in 2..5 {
                prob[y * 8 + x] = 0.9;
            }
        }
        let boxes = boxes_from_prob(&prob, 8, 8, 0.3, 1.0);
        assert_eq!(boxes.len(), 1);
        let b = boxes[0];
        assert!(b.x <= 2 && b.y <= 2);
        assert!(b.w >= 2 && b.h >= 2);
        assert!(b.score > 0.5);
    }

    #[test]
    fn ctc_peaks_report_where_each_character_hit() {
        // 4 steps: blank, 'a', blank, 'b' — the peaks carry the positions.
        let chars = vec!['a', 'b'];
        let logits = vec![
            0.9, 0.1, 0.0, // blank
            0.1, 0.8, 0.1, // a
            0.9, 0.1, 0.0, // blank
            0.1, 0.1, 0.9, // b
        ];
        let (text, conf, peaks) = ctc_decode_with_peaks(&logits, 4, 3, &chars);
        assert_eq!(text, "ab");
        assert!(conf > 0.0);
        assert_eq!(peaks.len(), 2);
        assert_eq!(peaks[0].ch, 'a');
        assert_eq!(peaks[0].step, 1);
        assert_eq!(peaks[1].ch, 'b');
        assert_eq!(peaks[1].step, 3);
    }

    #[test]
    fn glyph_width_follows_the_script() {
        // CJK glyphs are square, latin ones about half the line height.
        assert_eq!(char_width('数', 24), 24);
        assert_eq!(char_width('M', 24), 13);
    }

    #[test]
    fn noise_pixels_do_not_become_boxes() {
        // A single bright pixel is noise, not a text box.
        let mut prob = vec![0f32; 64];
        prob[10] = 0.9;
        assert!(boxes_from_prob(&prob, 8, 8, 0.3, 1.0).is_empty());
    }
}
