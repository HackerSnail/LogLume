//! The offline component package is verified before it is used (spec §8.1).
//!
//! The test builds a component in a temp dir (files + manifest) and checks the
//! parts that do not need the release signing key: the canonical payload is
//! reproducible, a tampered file no longer matches its checksum, and a
//! component is bound to one app version.

use lumelog_app_lib::ocr::component;
use std::fs;
use std::path::{Path, PathBuf};

fn temp_root(tag: &str) -> PathBuf {
    // One directory per test — the tests run in parallel.
    let dir = std::env::temp_dir().join(format!(
        "lumelog-ocr-comp-{}-{}",
        std::process::id(),
        tag
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("bin")).unwrap();
    fs::create_dir_all(dir.join("models")).unwrap();
    dir
}

fn sha256_hex(data: &[u8]) -> String {
    component::sha256_hex(data)
}

/// Minimal component: one runtime, two models, one dictionary.
fn build_component(root: &Path, app_version: &str) -> component::Manifest {
    let runtime_bytes = b"runtime-bytes".to_vec();
    let det_bytes = b"det-bytes".to_vec();
    let rec_bytes = b"rec-bytes".to_vec();
    let dict_bytes = b"dict-bytes".to_vec();
    fs::write(root.join("bin/onnxruntime.dll"), &runtime_bytes).unwrap();
    fs::write(root.join("models/det.onnx"), &det_bytes).unwrap();
    fs::write(root.join("models/rec.onnx"), &rec_bytes).unwrap();
    fs::write(root.join("models/dict.txt"), &dict_bytes).unwrap();

    let entry = |path: &str, data: &[u8]| component::FileEntry {
        path: path.to_string(),
        size: data.len() as u64,
        sha256: sha256_hex(data),
    };
    let runtime = entry("bin/onnxruntime.dll", &runtime_bytes);
    let det = entry("models/det.onnx", &det_bytes);
    let rec = entry("models/rec.onnx", &rec_bytes);
    let dict = entry("models/dict.txt", &dict_bytes);
    let payload = component::canonical_payload(
        app_version,
        "onnx",
        &runtime,
        &[det.clone(), rec.clone()],
        Some(&dict),
    );
    component::Manifest {
        version: "1.0.0".into(),
        app_version: app_version.into(),
        engine: "onnx".into(),
        runtime,
        models: vec![det, rec],
        dict: Some(dict),
        payload,
        // A dummy signature: the real one needs the private key, which lives
        // outside the repository. Signature rejection is covered by the unit
        // test "bad_signature_is_rejected".
        signature: "00".into(),
    }
}

#[test]
fn the_canonical_payload_is_reproducible() {
    let root = temp_root("payload");
    let manifest = build_component(root.as_path(), "0.1.0");
    let payload = component::canonical_payload(
        &manifest.app_version,
        &manifest.engine,
        &manifest.runtime,
        &manifest.models,
        manifest.dict.as_ref(),
    );
    assert_eq!(payload, manifest.payload, "payload must be reproducible");
    // The dictionary is part of the signed payload.
    assert!(payload.contains("dict=models/dict.txt"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_tampered_file_no_longer_matches_its_checksum() {
    let root = temp_root("tampered");
    let manifest = build_component(root.as_path(), "0.1.0");
    let rec = root.join("models/rec.onnx");
    fs::write(&rec, b"tampered").unwrap();
    let actual = component::file_sha256(&rec).unwrap();
    let expected = manifest
        .models
        .iter()
        .find(|m| m.path.contains("rec"))
        .map(|m| m.sha256.clone())
        .unwrap();
    assert_ne!(actual, expected, "a tampered file must not match the manifest");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_component_is_bound_to_one_app_version() {
    let root = temp_root("version");
    let manifest = build_component(root.as_path(), "0.1.0");
    assert_eq!(manifest.app_version, "0.1.0");
    // A component built for another app version produces a different payload,
    // so the signature cannot be reused across versions.
    let other = build_component(root.as_path(), "9.9.9");
    assert_ne!(other.payload, manifest.payload);
    let _ = fs::remove_dir_all(&root);
}
