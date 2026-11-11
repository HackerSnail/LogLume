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

use crate::ocr::component;
use crate::ocr::{DecodedImage, LanguageChoice, OcrError, Progress};
use lumelog_core::ocr::WordBox;

/// Detection thresholds (PP-OCR defaults).
const DET_THRESHOLD: f32 = 0.3;
const DET_UNCLIP_RATIO: f32 = 1.5;
/// Recognition input height; the width follows the box aspect ratio.
const REC_HEIGHT: usize = 48;
const REC_MAX_WIDTH: usize = 320;

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
pub fn recognize(
    image: &DecodedImage,
    _language: LanguageChoice,
    progress: Progress<'_>,
) -> Result<(Vec<WordBox>, String), OcrError> {
    progress("查找 OCR 组件包", 10);
    let comp = component::locate(component::COMPONENT_VERSION).map_err(|e| {
        OcrError::new("component_missing", format!("{}", e))
    })?;

    progress("加载 ONNX Runtime", 20);
    let runtime = comp.runtime_path();
    let env = ort::init_from(&runtime)
        .map_err(|e| OcrError::new("component_invalid", format!("加载 ONNX Runtime 失败：{}", e)))?
        .with_name("lumelog")
        .build()
        .map_err(|e| OcrError::new("component_invalid", format!("初始化推理环境失败：{}", e)))?;

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

    progress("检测文本区域", 35);
    let boxes = detect(&env, &det, image)?;

    progress("识别文字", 60);
    let words = recognize_boxes(&env, &rec, &chars, image, &boxes)?;

    Ok((words, "onnx".to_string()))
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

    let mut words = Vec::new();
    for b in boxes {
        let (crop, rw) = crop_resize(image, b);
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
        let (text, conf) = ctc_decode(data, steps, classes, chars);
        if text.trim().is_empty() {
            continue;
        }
        words.push(WordBox {
            text,
            x: b.x,
            y: b.y,
            w: b.w,
            h: b.h,
            conf: Some(conf),
        });
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
    fn noise_pixels_do_not_become_boxes() {
        // A single bright pixel is noise, not a text box.
        let mut prob = vec![0f32; 64];
        prob[10] = 0.9;
        assert!(boxes_from_prob(&prob, 8, 8, 0.3, 1.0).is_empty());
    }
}
