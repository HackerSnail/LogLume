"""Build the offline OCR component package (spec §8.1).

The component ships the PP-OCR models and the ONNX Runtime native library as
a separate, offline-installable package — the main installer stays small (P6)
and the app never downloads anything at runtime (industrial sites often have
no internet).

Input directory (staged by hand or by CI):

    stage/
      bin/onnxruntime.dll          (Linux .so / macOS .dylib)
      models/ch_PP-OCRv4_det_infer.onnx
      models/ch_PP-OCRv4_rec_infer.onnx
      models/ppocr_keys_v1.txt      (character dictionary for CTC decode)

Output:

    dist/ocr-component-<app-version>.zip
    dist/ocr-component-<app-version>/manifest.json

The manifest carries a SHA-256 per file and a detached Ed25519 signature over
a canonical payload. The private key never enters the repository: point
`LUMELOG_OCR_SIGNING_KEY` at a hex-encoded Ed25519 seed, or pass a key file.

Usage:

    python tools/build_ocr_component.py --stage path/to/stage --out dist

The public key the app checks against lives in
`app/src-tauri/src/ocr/component.rs` (`SIGNING_PUBLIC_KEY`); print the matching
public key with `--print-public-key`.
"""
import argparse
import hashlib
import json
import os
import shutil
import sys
import zipfile

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

# The canonical payload must match `lumelog_app_lib::ocr::component`.
PAYLOAD_VERSION = "lumelog-ocr-component/v1"


def sha256_of(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def file_entry(root: str, rel: str) -> dict:
    full = os.path.join(root, rel)
    return {
        "path": rel.replace("\\", "/"),
        "size": os.path.getsize(full),
        "sha256": sha256_of(full),
    }


def canonical_payload(app_version: str, engine: str, runtime: dict, models: list, dict_entry) -> str:
    lines = [PAYLOAD_VERSION, f"app={app_version}", f"engine={engine}", f"runtime={file_line(runtime)}"]
    for m in models:
        lines.append(f"model={file_line(m)}")
    if dict_entry:
        lines.append(f"dict={file_line(dict_entry)}")
    return "\n".join(lines) + "\n"


def file_line(entry: dict) -> str:
    return f"{entry['path']}|{entry['size']}|{entry['sha256']}"


def load_private_key(key_hex: str) -> Ed25519PrivateKey:
    seed = bytes.fromhex(key_hex.strip())
    if len(seed) != 32:
        raise SystemExit("签名密钥必须是 32 字节（64 个十六进制字符）")
    return Ed25519PrivateKey.from_private_bytes(seed)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--stage", help="staged component directory (bin/ + models/)")
    ap.add_argument("--out", default="dist", help="output directory (default: dist)")
    ap.add_argument("--app-version", default="0.1.0", help="app version the component binds to")
    ap.add_argument("--key", default=os.environ.get("LUMELOG_OCR_SIGNING_KEY", ""), help="hex Ed25519 seed")
    ap.add_argument("--key-file", help="file holding the hex Ed25519 seed")
    ap.add_argument("--print-public-key", action="store_true", help="print the public key and exit")
    args = ap.parse_args()

    key_hex = args.key
    if args.key_file:
        with open(args.key_file, "r", encoding="utf-8") as f:
            key_hex = f.read().strip()
    if not key_hex:
        print("需要签名密钥：用 --key / --key-file，或设置 LUMELOG_OCR_SIGNING_KEY", file=sys.stderr)
        return 2
    private = load_private_key(key_hex)
    public_hex = private.public_key().public_bytes_raw().hex()

    if args.print_public_key:
        print(public_hex)
        return 0
    if not args.stage:
        print("需要 --stage 目录", file=sys.stderr)
        return 2

    stage = os.path.abspath(args.stage)
    runtime_rel = None
    models = []
    dict_rel = None
    for root, _dirs, files in os.walk(stage):
        for name in sorted(files):
            rel = os.path.relpath(os.path.join(root, name), stage).replace("\\", "/")
            if rel.startswith("bin/"):
                runtime_rel = rel
            elif rel.endswith(".onnx"):
                models.append(rel)
            elif name.endswith(".txt") and "dict" in name.lower() or name.startswith("ppocr_keys"):
                dict_rel = rel
    if runtime_rel is None:
        print(f"{stage} 里没有 bin/ 下的 ONNX Runtime 动态库", file=sys.stderr)
        return 2
    det = [m for m in models if "det" in m]
    rec = [m for m in models if "rec" in m]
    if not det or not rec:
        print(f"{stage} 里需要检测模型（含 det）与识别模型（含 rec），现在有 {models}", file=sys.stderr)
        return 2

    runtime = file_entry(stage, runtime_rel)
    model_entries = [file_entry(stage, m) for m in det[:1] + rec[:1]]
    dict_entry = file_entry(stage, dict_rel) if dict_rel else None

    payload = canonical_payload(args.app_version, "onnx", runtime, model_entries, dict_entry)
    signature = private.sign(payload.encode("utf-8")).hex()

    manifest = {
        "version": "1.0.0",
        "app_version": args.app_version,
        "engine": "onnx",
        "runtime": runtime,
        "models": model_entries,
        "dict": dict_entry,
        "payload": payload,
        "signature": signature,
    }

    out_dir = os.path.abspath(args.out)
    pkg_dir = os.path.join(out_dir, f"ocr-component-{args.app_version}")
    if os.path.isdir(pkg_dir):
        shutil.rmtree(pkg_dir)
    os.makedirs(pkg_dir, exist_ok=True)

    for rel in [runtime_rel] + [m["path"] for m in model_entries] + ([dict_entry["path"]] if dict_entry else []):
        src = os.path.join(stage, rel)
        dst = os.path.join(pkg_dir, rel)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copy2(src, dst)

    with open(os.path.join(pkg_dir, "manifest.json"), "w", encoding="utf-8", newline="\n") as f:
        json.dump(manifest, f, ensure_ascii=False, indent=2)
        f.write("\n")

    zip_path = os.path.join(out_dir, f"ocr-component-{args.app_version}.zip")
    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as z:
        for root, _dirs, files in os.walk(pkg_dir):
            for name in sorted(files):
                full = os.path.join(root, name)
                z.write(full, os.path.relpath(full, pkg_dir).replace("\\", "/"))

    print(f"组件目录：{pkg_dir}")
    print(f"组件压缩包：{zip_path}（{os.path.getsize(zip_path) // 1024} KiB）")
    print(f"公钥（写进 component.rs 的 SIGNING_PUBLIC_KEY）：{public_hex}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
