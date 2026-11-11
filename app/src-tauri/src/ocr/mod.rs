//! Local OCR (spec §8, Phase 1): image → words → rows/columns → text.
//!
//! The engine is pluggable — Windows.Media.Ocr, macOS Vision, PP-ONNX — but all
//! of them report the same thing: word-level boxes. The column reconstruction
//! that turns boxes into a table lives in `lumelog_core::ocr` and is shared by
//! every engine.
//!
//! Nothing here calls a network service: the engines are either the OS built-in
//! one or the PP-ONNX models from the offline component package.

use image::ImageDecoder;
use lumelog_core::ocr::{grid_from_words, Grid, GridOptions, WordBox};
use serde::Serialize;
use std::path::Path;

pub mod component;
pub mod onnx;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod vision;

/// Which engine to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineChoice {
    /// System built-in engine if it supports the language, else the component.
    Auto,
    /// System built-in engine only.
    System,
    /// PP-ONNX from the offline component package only.
    Onnx,
}

/// Which language to recognize.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguageChoice {
    /// Follow the system, prefer Chinese.
    Auto,
    /// Simplified Chinese.
    ZhHans,
    /// English.
    English,
}

/// Everything that can go wrong; the kind is what the UI branches on.
#[derive(Debug, Clone, Serialize)]
pub struct OcrError {
    /// Stable machine-readable kind.
    pub kind: String,
    /// Human readable detail (Chinese, shown in a toast).
    pub message: String,
}

impl OcrError {
    fn new(kind: &str, message: impl Into<String>) -> Self {
        OcrError {
            kind: kind.to_string(),
            message: message.into(),
        }
    }

    fn decode(message: impl Into<String>) -> Self {
        OcrError::new("decode_failed", message)
    }

    fn engine(message: impl Into<String>) -> Self {
        OcrError::new("engine_failed", message)
    }
}

impl std::fmt::Display for OcrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl std::error::Error for OcrError {}

/// Request options.
#[derive(Debug, Clone, Copy)]
pub struct OcrOptions {
    pub engine: EngineChoice,
    pub language: LanguageChoice,
}

impl Default for OcrOptions {
    fn default() -> Self {
        OcrOptions {
            engine: EngineChoice::Auto,
            language: LanguageChoice::Auto,
        }
    }
}

/// The reconstructed table, flattened for the front end.
#[derive(Debug, Clone, Serialize)]
pub struct OcrTable {
    /// Rows of cell texts, top to bottom.
    pub rows: Vec<Vec<String>>,
    /// Number of detected columns.
    pub column_count: usize,
    /// Space padded table — the text that goes into the new tab.
    pub text: String,
    /// Tab separated table — machine readable variant.
    pub tsv: String,
}

impl OcrTable {
    fn from_grid(g: &Grid) -> Self {
        OcrTable {
            rows: g
                .rows
                .iter()
                .map(|row| row.iter().map(|c| c.text.clone()).collect())
                .collect(),
            column_count: g.column_count(),
            text: g.to_aligned_text(),
            tsv: g.to_tsv(),
        }
    }
}

/// Result of one recognition run.
#[derive(Debug, Clone, Serialize)]
pub struct OcrOutcome {
    /// Which engine actually ran ("windows", "vision", "onnx").
    pub engine: String,
    /// Language the engine used.
    pub language: String,
    /// Decoded image size in pixels.
    pub image_width: u32,
    pub image_height: u32,
    /// Word boxes in image pixel coordinates.
    pub words: Vec<WordBoxDto>,
    /// The reconstructed table.
    pub table: OcrTable,
}

/// Word box for the front end (the core type is not serializable).
#[derive(Debug, Clone, Serialize)]
pub struct WordBoxDto {
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// Engine confidence, if the engine reports one.
    pub conf: Option<f32>,
}

/// A decoded image plus the geometry the engines need.
#[derive(Debug)]
pub struct DecodedImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// BGRA8 pixels, top-down, 4 bytes per pixel.
    pub bgra: Vec<u8>,
}

/// Decode an image file and normalize its orientation.
///
/// EXIF orientation is applied here (not in the engines) so every engine and
/// every coordinate mapping works on the same pixel grid (spec §4.1).
pub fn decode_image(path: &Path) -> Result<DecodedImage, OcrError> {
    let reader = image::ImageReader::open(path)
        .map_err(|e| OcrError::decode(format!("打不开图片：{}", e)))?;
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| OcrError::decode(format!("无法识别图片格式：{}", e)))?;
    // EXIF orientation first (the decoder is consumed by the decode below).
    let orientation = decoder.orientation().ok();
    let mut img = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| OcrError::decode(format!("图片解码失败：{}", e)))?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    // BGRA8 is what both the Windows OCR engine and ONNX Runtime want.
    let mut bgra = Vec::with_capacity((width as usize) * (height as usize) * 4);
    for px in rgba.pixels() {
        bgra.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    Ok(DecodedImage {
        width,
        height,
        bgra,
    })
}

/// Progress callback: stage name and a 0..100 percentage.
pub type Progress<'a> = &'a mut dyn FnMut(&str, u8);

/// Recognize an image and rebuild its table.
pub fn recognize(
    path: &Path,
    opts: &OcrOptions,
    progress: Progress<'_>,
) -> Result<OcrOutcome, OcrError> {
    progress("解码图片", 5);
    let image = decode_image(path)?;

    progress("选择识别引擎", 15);
    let (words, engine, language) = run_engine(&image, opts, progress)?;

    progress("还原行列结构", 85);
    let grid = grid_from_words(&words, &GridOptions::default());
    let table = OcrTable::from_grid(&grid);
    progress("完成", 100);

    Ok(OcrOutcome {
        engine,
        language,
        image_width: image.width,
        image_height: image.height,
        words: words
            .into_iter()
            .map(|w| WordBoxDto {
                text: w.text,
                x: w.x,
                y: w.y,
                w: w.w,
                h: w.h,
                conf: w.conf,
            })
            .collect(),
        table,
    })
}

/// Engine selection per spec §3.2, with the fallback chain.
fn run_engine(
    image: &DecodedImage,
    opts: &OcrOptions,
    progress: Progress<'_>,
) -> Result<(Vec<WordBox>, String, String), OcrError> {
    match opts.engine {
        EngineChoice::System => {
            let (w, lang) = system_engine(image, opts.language, progress)?;
            Ok((w, system_engine_name(), lang))
        }
        EngineChoice::Onnx => {
            let (w, lang) = onnx::recognize(image, opts.language, progress)?;
            Ok((w, "onnx".to_string(), lang))
        }
        EngineChoice::Auto => {
            // System engine first when it speaks the wanted language.
            if let Ok((w, lang)) = system_engine(image, opts.language, progress) {
                return Ok((w, system_engine_name(), lang));
            }
            // Then the component (the guaranteed path).
            if let Ok((w, lang)) = onnx::recognize(image, opts.language, progress) {
                return Ok((w, "onnx".to_string(), lang));
            }
            // Then the system engine with whatever language it does have.
            let fallback = OcrOptions {
                engine: EngineChoice::System,
                language: LanguageChoice::Auto,
            };
            let (w, lang) = system_engine(image, fallback.language, progress)?;
            Ok((w, system_engine_name(), lang))
        }
    }
}

/// Name of the platform engine ("windows" / "vision" / "none").
pub fn system_engine_name() -> String {
    #[cfg(target_os = "windows")]
    {
        "windows".to_string()
    }
    #[cfg(target_os = "macos")]
    {
        "vision".to_string()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        "none".to_string()
    }
}

/// Run the platform's built-in OCR engine.
#[allow(unused_variables)]
fn system_engine(
    image: &DecodedImage,
    language: LanguageChoice,
    progress: Progress<'_>,
) -> Result<(Vec<WordBox>, String), OcrError> {
    #[cfg(target_os = "windows")]
    {
        windows::recognize(image, language, progress)
    }
    #[cfg(target_os = "macos")]
    {
        vision::recognize(image, language, progress)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (image, language, progress);
        Err(OcrError::new(
            "no_engine",
            "本平台没有系统内置 OCR，请安装 OCR 离线组件包",
        ))
    }
}

/// Languages the platform engine can be asked for, in preference order.
pub fn language_candidates(language: LanguageChoice) -> Vec<&'static str> {
    match language {
        LanguageChoice::ZhHans => vec!["zh-Hans", "zh-CN", "en-US"],
        LanguageChoice::English => vec!["en-US"],
        LanguageChoice::Auto => vec!["zh-Hans", "zh-CN", "en-US"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_preference_order() {
        assert_eq!(
            language_candidates(LanguageChoice::ZhHans)[0],
            "zh-Hans",
            "Chinese is preferred first"
        );
        assert_eq!(language_candidates(LanguageChoice::English).len(), 1);
        assert_eq!(language_candidates(LanguageChoice::Auto).len(), 3);
    }

    #[test]
    fn table_carries_rows_columns_and_text() {
        let words = vec![
            WordBox {
                text: "ID".into(),
                x: 10,
                y: 0,
                w: 16,
                h: 20,
                conf: Some(0.9),
            },
            WordBox {
                text: "Value".into(),
                x: 120,
                y: 0,
                w: 40,
                h: 20,
                conf: Some(0.9),
            },
            WordBox {
                text: "0x01".into(),
                x: 10,
                y: 24,
                w: 32,
                h: 20,
                conf: Some(0.9),
            },
            WordBox {
                text: "1200".into(),
                x: 120,
                y: 24,
                w: 32,
                h: 20,
                conf: Some(0.9),
            },
        ];
        let g = lumelog_core::ocr::grid(&words);
        let t = OcrTable::from_grid(&g);
        assert_eq!(t.column_count, 2);
        assert_eq!(t.rows.len(), 2);
        assert!(t.text.contains("1200"));
        assert!(t.tsv.contains("ID\tValue"));
    }

    #[test]
    fn missing_image_is_a_decode_error() {
        let err = decode_image(Path::new("definitely-not-here.png")).unwrap_err();
        assert_eq!(err.kind, "decode_failed");
    }
}
