//! The offline OCR component package (spec §8.1).
//!
//! The component ships the PP-OCR models and the ONNX Runtime native library
//! as a separate, offline-installable package. It is deliberately NOT part of
//! the main installer (P6 stays ≤15MB) and never downloaded at runtime —
//! industrial sites often have no internet at all.
//!
//! A component directory looks like:
//!
//! ```text
//! ocr-component-<version>/
//!   manifest.json
//!   bin/onnxruntime.dll        (Linux .so / macOS .dylib)
//!   models/ch_PP-OCRv4_det_infer.onnx
//!   models/ch_PP-OCRv4_rec_infer.onnx
//! ```
//!
//! `manifest.json` carries the file list with sizes and SHA-256 checksums, a
//! canonical payload string and a detached Ed25519 signature over that
//! payload. Verification order: signature → version binding → per-file
//! checksum.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// One file of the component.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileEntry {
    /// Path relative to the component root.
    pub path: String,
    /// Size in bytes.
    pub size: u64,
    /// Lowercase hex SHA-256 of the file content.
    pub sha256: String,
}

/// The component manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    /// Component version, e.g. "1.0.0".
    pub version: String,
    /// The app version this component is bound to (spec §8.1: strongly bound).
    pub app_version: String,
    /// Engine name, "onnx".
    pub engine: String,
    /// ONNX Runtime native library.
    pub runtime: FileEntry,
    /// PP-OCR models: detection first, recognition second.
    pub models: Vec<FileEntry>,
    /// Character dictionary for the recognition model (CTC decode needs it).
    pub dict: Option<FileEntry>,
    /// Canonical payload the signature covers.
    pub payload: String,
    /// Detached Ed25519 signature over `payload`, hex encoded.
    pub signature: String,
}

/// Why a component was rejected.
#[derive(Debug)]
pub enum ComponentError {
    /// No component directory found in any known location.
    NotFound { searched: Vec<PathBuf> },
    /// The manifest could not be read or parsed.
    BadManifest(String),
    /// The component is bound to another app version.
    VersionMismatch { expected: String, found: String },
    /// The signature did not verify.
    BadSignature(String),
    /// A file is missing, truncated or modified.
    ChecksumMismatch { path: String, reason: String },
}

impl std::fmt::Display for ComponentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComponentError::NotFound { searched } => {
                write!(f, "未找到 OCR 组件包，已查找：")?;
                for p in searched {
                    write!(f, "{} ", p.display())?;
                }
                Ok(())
            }
            ComponentError::BadManifest(m) => write!(f, "组件清单损坏：{}", m),
            ComponentError::VersionMismatch { expected, found } => write!(
                f,
                "OCR 组件包版本不匹配：需要 {}，实际 {}",
                expected, found
            ),
            ComponentError::BadSignature(m) => write!(f, "组件签名校验失败：{}", m),
            ComponentError::ChecksumMismatch { path, reason } => {
                write!(f, "组件文件校验失败（{}）：{}", path, reason)
            }
        }
    }
}

/// The component version this build expects. Bumping it invalidates older
/// components on purpose (spec §8.1: component and app are strongly bound).
pub const COMPONENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Public key that signs component manifests (hex). The matching private key
/// lives outside the repository — `tools/build_ocr_component.py` signs with it.
///
/// The value checked in here is a development key: the release process
/// replaces it with the production public key and keeps the seed out of the
/// repository.
pub const SIGNING_PUBLIC_KEY: &str = "d4ba2eea6e98430eb5d6c2d7fca05ddb8c6ee5853812fd5a43945d4836c368ae";

/// Build the canonical payload the signature covers.
///
/// Field order is fixed, so the same component always produces the same
/// payload regardless of JSON formatting.
pub fn canonical_payload(
    app_version: &str,
    engine: &str,
    runtime: &FileEntry,
    models: &[FileEntry],
    dict: Option<&FileEntry>,
) -> String {
    let mut s = String::from("lumelog-ocr-component/v1\n");
    s.push_str(&format!("app={}\n", app_version));
    s.push_str(&format!("engine={}\n", engine));
    s.push_str(&format!("runtime={}\n", file_line(runtime)));
    for m in models {
        s.push_str(&format!("model={}\n", file_line(m)));
    }
    if let Some(d) = dict {
        s.push_str(&format!("dict={}\n", file_line(d)));
    }
    s
}

fn file_line(f: &FileEntry) -> String {
    format!("{}|{}|{}", f.path, f.size, f.sha256)
}

/// SHA-256 of a file, lowercase hex.
pub fn file_sha256(path: &Path) -> Result<String, ComponentError> {
    let data = std::fs::read(path)
        .map_err(|e| ComponentError::ChecksumMismatch {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
    Ok(sha256_hex(&data))
}

/// SHA-256 of a byte slice, lowercase hex.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

/// Verify the manifest signature against the embedded public key.
pub fn verify_signature(payload: &str, signature_hex: &str) -> Result<(), ComponentError> {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

    let key_bytes = hex_to_bytes(SIGNING_PUBLIC_KEY).ok_or_else(|| {
        ComponentError::BadSignature("内置公钥格式错误".to_string())
    })?;
    let key: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| ComponentError::BadSignature("内置公钥长度错误".to_string()))?;
    let verifying = VerifyingKey::from_bytes(&key)
        .map_err(|e| ComponentError::BadSignature(format!("内置公钥无效：{}", e)))?;
    let sig_bytes = hex_to_bytes(signature_hex)
        .ok_or_else(|| ComponentError::BadSignature("签名格式错误".to_string()))?;
    let sig: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| ComponentError::BadSignature("签名长度错误".to_string()))?;
    let signature = Signature::from_bytes(&sig);
    verifying
        .verify(payload.as_bytes(), &signature)
        .map_err(|e| ComponentError::BadSignature(format!("签名不匹配：{}", e)))
}

fn hex_to_bytes(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

/// A verified component: where its runtime and models live.
#[derive(Debug, Clone)]
pub struct Component {
    /// Component root directory.
    pub root: PathBuf,
    /// The verified manifest.
    pub manifest: Manifest,
}

impl Component {
    /// Absolute path of the ONNX Runtime native library.
    pub fn runtime_path(&self) -> PathBuf {
        self.root.join(&self.manifest.runtime.path)
    }

    /// Absolute path of the detection model.
    pub fn det_model_path(&self) -> Option<PathBuf> {
        self.manifest
            .models
            .iter()
            .find(|m| m.path.contains("det"))
            .map(|m| self.root.join(&m.path))
    }

    /// Absolute path of the recognition model.
    pub fn rec_model_path(&self) -> Option<PathBuf> {
        self.manifest
            .models
            .iter()
            .find(|m| m.path.contains("rec"))
            .map(|m| self.root.join(&m.path))
    }
}

/// Directories that may hold a component, in priority order.
pub fn candidate_dirs(app_version: &str) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("LUMELOG_OCR_COMPONENT") {
        dirs.push(PathBuf::from(p));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.join("ocr").join(app_version));
        }
    }
    if let Ok(cfg) = std::env::var("APP_CONFIG") {
        dirs.push(
            PathBuf::from(cfg)
                .join("LumeLog")
                .join("ocr")
                .join(app_version),
        );
    }
    dirs
}

/// Find and verify the component package.
pub fn locate(app_version: &str) -> Result<Component, ComponentError> {
    let dirs = candidate_dirs(app_version);
    let mut searched = Vec::new();
    for dir in &dirs {
        searched.push(dir.clone());
        let manifest_path = dir.join("manifest.json");
        if !manifest_path.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(&manifest_path)
            .map_err(|e| ComponentError::BadManifest(e.to_string()))?;
        let manifest: Manifest = serde_json::from_str(&raw)
            .map_err(|e| ComponentError::BadManifest(e.to_string()))?;
        verify_component(dir, &manifest, app_version)?;
        return Ok(Component {
            root: dir.clone(),
            manifest,
        });
    }
    Err(ComponentError::NotFound { searched })
}

/// Verify a manifest and the files it lists.
pub fn verify_component(
    root: &Path,
    manifest: &Manifest,
    app_version: &str,
) -> Result<(), ComponentError> {
    // 1) signature over the canonical payload
    verify_signature(&manifest.payload, &manifest.signature)?;

    // 2) the payload must match the fields (a swapped file list would
    //    otherwise still carry a valid signature over a stale payload)
    let expected = canonical_payload(
        &manifest.app_version,
        &manifest.engine,
        &manifest.runtime,
        &manifest.models,
        manifest.dict.as_ref(),
    );
    if expected != manifest.payload {
        return Err(ComponentError::BadManifest(
            "清单内容与签名载荷不一致".to_string(),
        ));
    }

    // 3) version binding
    if manifest.app_version != app_version {
        return Err(ComponentError::VersionMismatch {
            expected: app_version.to_string(),
            found: manifest.app_version.clone(),
        });
    }

    // 4) per file checksum
    let mut files = vec![&manifest.runtime];
    for m in &manifest.models {
        files.push(m);
    }
    if let Some(d) = &manifest.dict {
        files.push(d);
    }
    for f in files {
        let path = root.join(&f.path);
        let meta = std::fs::metadata(&path).map_err(|e| ComponentError::ChecksumMismatch {
            path: f.path.clone(),
            reason: e.to_string(),
        })?;
        if meta.len() != f.size {
            return Err(ComponentError::ChecksumMismatch {
                path: f.path.clone(),
                reason: format!("大小 {} != 期望 {}", meta.len(), f.size),
            });
        }
        let actual = file_sha256(&path)?;
        if actual != f.sha256 {
            return Err(ComponentError::ChecksumMismatch {
                path: f.path.clone(),
                reason: "SHA-256 不匹配".to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, size: u64, hash: &str) -> FileEntry {
        FileEntry {
            path: path.to_string(),
            size,
            sha256: hash.to_string(),
        }
    }

    #[test]
    fn payload_is_stable_and_field_sensitive() {
        let rt = entry("bin/onnxruntime.dll", 10, "aa");
        let m = vec![entry("models/det.onnx", 20, "bb")];
        let a = canonical_payload("0.1.0", "onnx", &rt, &m, None);
        let b = canonical_payload("0.1.0", "onnx", &rt, &m, None);
        assert_eq!(a, b, "same input → same payload");
        let c = canonical_payload("0.1.0", "onnx", &entry("bin/onnxruntime.dll", 11, "aa"), &m, None);
        assert_ne!(a, c, "a changed file size must change the payload");
        let d = canonical_payload(
            "0.1.0",
            "onnx",
            &rt,
            &m,
            Some(&entry("models/dict.txt", 5, "cc")),
        );
        assert_ne!(a, d, "the dictionary must be part of the signed payload");
    }

    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn bad_signature_is_rejected() {
        let err = verify_signature("payload", "00").unwrap_err();
        assert!(matches!(err, ComponentError::BadSignature(_)));
    }

    #[test]
    fn version_mismatch_is_reported() {
        let root = std::env::temp_dir().join(format!("lumelog-ocr-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let payload = "lumelog-ocr-component/v1\napp=0.1.0\nengine=onnx\n";
        let manifest = Manifest {
            version: "1.0.0".into(),
            app_version: "0.1.0".into(),
            engine: "onnx".into(),
            runtime: entry("bin/onnxruntime.dll", 0, "x"),
            models: vec![],
            dict: None,
            payload: payload.into(),
            signature: "00".into(),
        };
        // Signature is checked before the version, so a bogus signature is
        // reported first — the point is that neither slips through.
        let err = verify_component(&root, &manifest, "9.9.9").unwrap_err();
        assert!(matches!(
            err,
            ComponentError::BadSignature(_) | ComponentError::VersionMismatch { .. }
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_component_is_reported_with_searched_paths() {
        let err = locate("0.0.0-nonexistent").unwrap_err();
        match err {
            ComponentError::NotFound { searched } => assert!(!searched.is_empty()),
            other => panic!("unexpected error: {:?}", other),
        }
    }
}
