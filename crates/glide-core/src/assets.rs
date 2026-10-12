//! Local media storage.
//!
//! Documents never embed media. Files live in an `assets/` folder next to the
//! JSON document and are referenced by relative, forward-slash paths. Imported
//! files are named by a hash of their content, so importing the same image
//! twice stores it once and documents sharing a folder never collide.

use crate::document::Document;
use std::io;
use std::path::{Path, PathBuf};

pub const ASSETS_DIR: &str = "assets";

/// FNV-1a 64-bit: stable across platforms and Rust versions.
pub fn content_hash(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn sanitize_ext(ext: &str) -> String {
    let e: String = ext.trim_start_matches('.').chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if e.is_empty() { "bin".into() } else { e.to_ascii_lowercase() }
}

/// Stores `bytes` in `<base_dir>/assets/` and returns the relative reference.
pub fn import_bytes(base_dir: &Path, bytes: &[u8], ext: &str) -> io::Result<String> {
    let name = format!("{:016x}.{}", content_hash(bytes), sanitize_ext(ext));
    let dir = base_dir.join(ASSETS_DIR);
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(&name);
    if !dest.exists() {
        std::fs::write(&dest, bytes)?;
    }
    Ok(format!("{ASSETS_DIR}/{name}"))
}

/// Copies a file into the document's asset folder. Large files (videos) are
/// hashed and copied in a streaming fashion.
pub fn import_file(base_dir: &Path, src: &Path) -> io::Result<String> {
    use std::io::Read;
    let ext = sanitize_ext(src.extension().and_then(|e| e.to_str()).unwrap_or("bin"));
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut file = std::fs::File::open(src)?;
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        for b in &buf[..n] {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    let name = format!("{h:016x}.{ext}");
    let dir = base_dir.join(ASSETS_DIR);
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(&name);
    if !dest.exists() {
        std::fs::copy(src, &dest)?;
    }
    Ok(format!("{ASSETS_DIR}/{name}"))
}

/// Resolves a document-relative media reference to a filesystem path.
pub fn resolve(base_dir: Option<&Path>, reference: &str) -> PathBuf {
    let rel = Path::new(reference);
    match base_dir {
        Some(base) if rel.is_relative() => base.join(rel),
        _ => rel.to_path_buf(),
    }
}

/// Copies every media file referenced by `doc` from `from` to `to` (used when
/// a document is saved to a different folder). Returns references that could
/// not be found.
pub fn relocate(doc: &Document, from: &Path, to: &Path) -> io::Result<Vec<String>> {
    let mut missing = Vec::new();
    if same_dir(from, to) {
        return Ok(missing);
    }
    for reference in doc.media_paths() {
        let rel = Path::new(&reference);
        if rel.is_absolute() {
            continue;
        }
        let src = from.join(rel);
        let dst = to.join(rel);
        if dst.exists() {
            continue;
        }
        if !src.exists() {
            missing.push(reference.clone());
            continue;
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&src, &dst)?;
    }
    Ok(missing)
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_is_content_addressed_and_relative() {
        let dir = std::env::temp_dir().join(format!("glide-assets-test-{}", std::process::id()));
        let a = import_bytes(&dir, b"hello", ".PNG").unwrap();
        let b = import_bytes(&dir, b"hello", "png").unwrap();
        assert_eq!(a, b);
        assert!(a.starts_with("assets/") && a.ends_with(".png"));
        assert!(resolve(Some(&dir), &a).exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
