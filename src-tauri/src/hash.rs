//! 哈希工具（阶段 5）：数据摘要、字体摘要、模板包校验和。

use std::path::Path;

use sha2::{Digest, Sha256};

/// SHA-256 十六进制摘要。
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex_encode(&hasher.finalize())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// 递归计算目录内容的校验和（按相对路径排序，逐文件累加 路径:大小:内容）。
/// 模板包「发布后不可变」的判定依据。
pub fn dir_checksum(dir: &Path) -> Option<String> {
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    collect_files(dir, dir, &mut files);
    if files.is_empty() {
        return None;
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hasher = Sha256::new();
    for (rel, path) in files {
        let bytes = std::fs::read(&path).ok()?;
        hasher.update(rel.as_bytes());
        hasher.update(&bytes.len().to_le_bytes());
        hasher.update(&bytes);
    }
    Some(hex_encode(&hasher.finalize()))
}

use std::path::PathBuf;

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if path.is_dir() {
            collect_files(root, &path, out);
        } else {
            out.push((rel, path));
        }
    }
}

/// 字体目录摘要：仅按「文件名 + 大小」计算（不读字体内容，速度快）。
pub fn font_dir_hash(dir: &Path) -> String {
    if !dir.is_dir() {
        return "none".to_string();
    }
    let mut files: Vec<(String, u64)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                files.push((
                    entry.file_name().to_string_lossy().to_string(),
                    entry.metadata().map(|m| m.len()).unwrap_or(0),
                ));
            }
        }
    }
    if files.is_empty() {
        return "none".to_string();
    }
    files.sort();
    let mut hasher = Sha256::new();
    for (name, size) in files {
        hasher.update(name.as_bytes());
        hasher.update(&size.to_le_bytes());
    }
    format!("fonts-{}", &hex_encode(&hasher.finalize())[..16])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_is_stable() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn dir_checksum_changes_with_content() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"one").unwrap();
        let c1 = dir_checksum(dir.path()).unwrap();
        std::fs::write(dir.path().join("a.txt"), b"two").unwrap();
        let c2 = dir_checksum(dir.path()).unwrap();
        assert_ne!(c1, c2);
    }

    #[test]
    fn empty_dir_hash_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(dir_checksum(dir.path()).is_none());
        assert_eq!(font_dir_hash(dir.path()), "none");
    }
}
