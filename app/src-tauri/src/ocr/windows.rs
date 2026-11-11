//! Windows built-in OCR (Windows.Media.Ocr) — spec §3.1.
//!
//! The engine gives word boxes but no confidence (spec §5.1) and reports the
//! boxes in the **deskewed** frame: `OcrResult.TextAngle` is the rotation the
//! engine applied, so the boxes have to be rotated back around the image
//! center to land in the original pixel grid (spec §4.2).

use crate::ocr::{language_candidates, DecodedImage, LanguageChoice, OcrError, Progress};
use lumelog_core::ocr::WordBox;
use std::future::IntoFuture;
use windows::core::Interface;
use windows::Foundation::IPropertyValue;
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::{OcrEngine, OcrResult};
use windows::Storage::Streams::Buffer;
use windows::Win32::System::WinRT::IBufferByteAccess;

/// Recognize an image with the Windows OCR engine.
pub fn recognize(
    image: &DecodedImage,
    language: LanguageChoice,
    progress: Progress<'_>,
) -> Result<(Vec<WordBox>, String), OcrError> {
    let (engine, tag) = pick_engine(language)?;

    progress("准备位图", 25);
    let bitmap = software_bitmap(image)?;

    progress("识别中", 45);
    let operation = engine
        .RecognizeAsync(&bitmap)
        .map_err(|e| OcrError::engine(format!("识别启动失败：{}", e)))?;
    // WinRT hands back an async operation; the OCR runs on a blocking thread,
    // so the future is awaited here with Tauri's runtime helper.
    let result: OcrResult = tauri::async_runtime::block_on(operation.into_future())
        .map_err(|e| OcrError::engine(format!("识别失败：{}", e)))?;

    // TextAngle is an IReference<f64>; read it through IPropertyValue.
    let angle = match result.TextAngle() {
        Ok(reference) => match reference.cast::<IPropertyValue>() {
            Ok(pv) => pv.GetDouble().unwrap_or(0.0),
            Err(_) => 0.0,
        },
        Err(_) => 0.0,
    };
    let words = collect_words(&result, angle, image.width, image.height);
    Ok((words, tag))
}

/// Build a BGRA8 SoftwareBitmap from the decoded pixels.
fn software_bitmap(image: &DecodedImage) -> Result<SoftwareBitmap, OcrError> {
    let len = image.bgra.len() as u32;
    let buffer = Buffer::Create(len)
        .map_err(|e| OcrError::engine(format!("创建缓冲区失败：{}", e)))?;
    buffer
        .SetLength(len)
        .map_err(|e| OcrError::engine(format!("设置缓冲区长度失败：{}", e)))?;
    {
        // IBufferByteAccess hands out the raw pointer; the copy is bounded by
        // the length we just set.
        let access: IBufferByteAccess = buffer
            .cast()
            .map_err(|e| OcrError::engine(format!("无法写入缓冲区：{}", e)))?;
        let ptr = unsafe {
            access
                .Buffer()
                .map_err(|e| OcrError::engine(format!("无法取到缓冲区指针：{}", e)))?
        };
        if !ptr.is_null() {
            unsafe {
                std::ptr::copy_nonoverlapping(image.bgra.as_ptr(), ptr, image.bgra.len());
            }
        }
    }
    SoftwareBitmap::CreateCopyFromBuffer(
        &buffer,
        BitmapPixelFormat::Bgra8,
        image.width as i32,
        image.height as i32,
    )
    .map_err(|e| OcrError::engine(format!("创建位图失败：{}", e)))
}

/// Pick the engine for the wanted language, falling back to what is installed.
fn pick_engine(language: LanguageChoice) -> Result<(OcrEngine, String), OcrError> {
    let mut last: Option<String> = None;
    for tag in language_candidates(language) {
        let lang = match Language::CreateLanguage(&windows::core::HSTRING::from(tag)) {
            Ok(l) => l,
            Err(e) => {
                last = Some(format!("语言标签 {} 无效：{}", tag, e));
                continue;
            }
        };
        match OcrEngine::IsLanguageSupported(&lang) {
            Ok(true) => {}
            Ok(false) => {
                last = Some(format!("系统未安装 {} 的 OCR 语言包", tag));
                continue;
            }
            Err(e) => {
                last = Some(format!("检查 {} 语言包失败：{}", tag, e));
                continue;
            }
        }
        match OcrEngine::TryCreateFromLanguage(&lang) {
            Ok(engine) => return Ok((engine, tag.to_string())),
            Err(e) => {
                last = Some(format!("创建 {} 识别引擎失败：{}", tag, e));
                continue;
            }
        }
    }
    Err(OcrError::new(
        "no_engine",
        match last {
            Some(m) => format!("Windows OCR 不可用：{}", m),
            None => "Windows OCR 不可用".to_string(),
        },
    ))
}

/// Collect the words, mapping the boxes back into the original image frame.
fn collect_words(
    result: &OcrResult,
    angle: f64,
    width: u32,
    height: u32,
) -> Vec<WordBox> {
    let mut words = Vec::new();
    let lines = match result.Lines() {
        Ok(l) => l,
        Err(_) => return words,
    };
    for line in lines {
        let ws = match line.Words() {
            Ok(w) => w,
            Err(_) => continue,
        };
        for word in ws {
            let r = match word.BoundingRect() {
                Ok(r) => r,
                Err(_) => continue,
            };
            let text = word.Text().map(|t| t.to_string()).unwrap_or_default();
            let (x, y) = rotate_back(
                r.X as f64,
                r.Y as f64,
                angle,
                width as f64,
                height as f64,
            );
            let (w, h) = rotate_extent(r.Width as f64, r.Height as f64, angle);
            words.push(WordBox {
                text,
                x: x.round() as i32,
                y: y.round() as i32,
                w: w.round() as i32,
                h: h.round() as i32,
                // Windows OCR has no per-word confidence (spec §5.1).
                conf: None,
            });
        }
    }
    words
}

/// Rotate a point back into the original frame (inverse of the engine's
/// deskew), around the image center.
fn rotate_back(x: f64, y: f64, angle: f64, width: f64, height: f64) -> (f64, f64) {
    if angle.abs() < 1e-6 {
        return (x, y);
    }
    let (cx, cy) = (width / 2.0, height / 2.0);
    let dx = x - cx;
    let dy = y - cy;
    let s = angle.sin();
    let c = angle.cos();
    // Inverse rotation: the engine rotated by `angle`, so undo it.
    (cx + dx * c + dy * s, cy - dx * s + dy * c)
}

/// Axis aligned extent of a box after the inverse rotation.
fn rotate_extent(w: f64, h: f64, angle: f64) -> (f64, f64) {
    if angle.abs() < 1e-6 {
        return (w, h);
    }
    let s = angle.sin();
    let c = angle.cos();
    let rw = (w * c).abs() + (h * s).abs();
    let rh = (w * s).abs() + (h * c).abs();
    (rw, rh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upright_boxes_are_untouched() {
        let (x, y) = rotate_back(10.0, 20.0, 0.0, 100.0, 50.0);
        assert_eq!((x, y), (10.0, 20.0));
        assert_eq!(rotate_extent(8.0, 4.0, 0.0), (8.0, 4.0));
    }

    #[test]
    fn rotated_boxes_move_around_the_center() {
        // The engine deskews by +90°, so undoing it turns the point right of
        // the center into one above it: (30, 0) → (0, -30) around (50, 25).
        let (x, y) = rotate_back(80.0, 25.0, std::f64::consts::FRAC_PI_2, 100.0, 50.0);
        assert!((x - 50.0).abs() < 1e-6);
        assert!((y + 5.0).abs() < 1e-6);
    }

    #[test]
    fn rotated_extent_grows_to_the_bounding_box() {
        let (w, h) = rotate_extent(10.0, 4.0, std::f64::consts::FRAC_PI_2);
        assert!((w - 4.0).abs() < 1e-6);
        assert!((h - 10.0).abs() < 1e-6);
    }
}
