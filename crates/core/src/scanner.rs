// Scanner: 目录扫描（快速，不含大小）
use crate::models::{DirectoryMeta, ScanError};
use std::path::Path;

pub fn scan_paths(paths: &[&Path]) -> (Vec<DirectoryMeta>, Vec<ScanError>) {
    scan_directories(paths, true)
}

/// GUI 目录浏览：列出所有直接子目录，不递归计算大小。
pub fn browse_paths(paths: &[&Path]) -> (Vec<DirectoryMeta>, Vec<ScanError>) {
    scan_directories(paths, false)
}

fn scan_directories(paths: &[&Path], hidden_only: bool) -> (Vec<DirectoryMeta>, Vec<ScanError>) {
    let mut dirs = Vec::new();
    let mut errors = Vec::new();

    for &base_path in paths {
        if let Ok(entries) = std::fs::read_dir(base_path) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                let is_directory = path.is_dir();
                if !hidden_only || is_directory {
                    let name = match path.file_name().and_then(|n| n.to_str()) {
                        Some(name) if !hidden_only || name.starts_with('.') => name.to_string(),
                        _ => continue,
                    };

                    let metadata = entry.metadata().ok();
                    let last_modified = metadata
                        .as_ref()
                        .and_then(|m| m.modified().ok())
                        .map(|t| DateTime::from(t))
                        .unwrap_or_else(|| Utc::now());

                    let top_level_samples =
                        if is_directory && !crate::path_utils::is_private_name(&name) {
                            sample_top_files(&path, 20)
                        } else {
                            Vec::new()
                        };

                    dirs.push(DirectoryMeta {
                        path,
                        name,
                        is_directory,
                        size: metadata.map_or(0, |metadata| metadata.len()),
                        last_modified,
                        top_level_samples,
                        bundle_id_hint: None,
                    });
                }
            }
        } else {
            errors.push(ScanError::PermissionDenied {
                path: base_path.to_path_buf(),
            });
        }
    }

    (dirs, errors)
}

/// 异步计算目录大小（批量 du）
pub async fn calculate_sizes(dirs: &[DirectoryMeta]) -> Vec<u64> {
    if dirs.is_empty() {
        return vec![];
    }

    // 克隆路径以避免借用问题（需要 'static）
    let paths: Vec<std::path::PathBuf> = dirs.iter().map(|d| d.path.clone()).collect();

    // 在线程池中运行 du（避免阻塞）
    let sizes = tokio::task::spawn_blocking(move || batch_dir_size(&paths))
        .await
        .ok()
        .unwrap_or_default();

    sizes
}

/// 批量 du 调用（同步）
fn batch_dir_size(paths: &[std::path::PathBuf]) -> Vec<u64> {
    if paths.is_empty() {
        return vec![];
    }

    let output = match std::process::Command::new("du")
        .arg("-sk")
        .args(paths)
        .output()
    {
        Ok(o) => o,
        Err(_) => return vec![0; paths.len()],
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut size_map: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

    for line in stdout.lines() {
        let line = line.trim();
        if let Some(space_idx) = line.find(char::is_whitespace) {
            let size_str = &line[..space_idx];
            let path_str = line[space_idx..].trim();

            if let Ok(size_kb) = size_str.parse::<u64>() {
                let normalized = normalize_path(path_str);
                size_map.insert(normalized, size_kb * 1024);
            }
        }
    }

    paths
        .iter()
        .map(|p| {
            let path_str = p.to_str().unwrap_or("");
            let canon = std::fs::canonicalize(p).ok();

            // 直接匹配
            if let Some(&size) = size_map.get(path_str) {
                return size;
            }

            // 规范化路径匹配
            if let Some(ref canon_path) = canon {
                let canon_str = canon_path.to_str().unwrap_or("");
                if let Some(&size) = size_map.get(canon_str) {
                    return size;
                }
            }

            // 文件名匹配（兜底）
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                for (k, &v) in &size_map {
                    if k.ends_with(name) {
                        return v;
                    }
                }
            }

            0
        })
        .collect()
}

fn normalize_path(path: &str) -> String {
    if path.starts_with('~') {
        std::env::var("HOME")
            .map(|home| path.replacen('~', &home, 1))
            .unwrap_or_else(|_| path.to_string())
    } else {
        path.to_string()
    }
}

use chrono::{DateTime, Utc};

fn sample_top_files(path: &Path, limit: usize) -> Vec<String> {
    std::fs::read_dir(path)
        .ok()
        .and_then(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .take(limit)
                .filter_map(|e| e.file_name().into_string().ok())
                .collect::<Vec<_>>()
                .into()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{browse_paths, scan_paths};
    use std::fs;

    #[test]
    fn browse_lists_regular_and_hidden_directories() {
        let root = std::env::temp_dir().join(format!(
            "dirdetective-scanner-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("regular")).unwrap();
        fs::create_dir_all(root.join(".hidden")).unwrap();
        fs::write(root.join("readme.txt"), "test").unwrap();
        fs::create_dir_all(root.join("Documents")).unwrap();
        fs::write(root.join("Documents").join("private-resume.pdf"), "private").unwrap();

        let (browse, _) = browse_paths(&[root.as_path()]);
        let (scan, _) = scan_paths(&[root.as_path()]);
        let documents = browse.iter().find(|item| item.name == "Documents").unwrap();
        assert!(documents.top_level_samples.is_empty());

        assert_eq!(browse.len(), 4);
        assert!(browse.iter().any(|entry| !entry.is_directory));
        assert_eq!(scan.len(), 1);
        assert_eq!(scan[0].name, ".hidden");

        fs::remove_dir_all(root).unwrap();
    }
}
