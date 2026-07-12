use dirdetective_core::models::{EvidencePool, Extension, InstalledApp, Package, ProcessInfo};
use std::path::Path;

/// 证据收集器 trait（跨平台接口）
pub trait EvidenceCollector: Send + Sync {
    /// 收集所有证据
    fn collect(&self) -> EvidencePool;
}

/// macOS 证据收集器（v0.1 实现）
pub struct MacCollector;

impl EvidenceCollector for MacCollector {
    fn collect(&self) -> EvidencePool {
        let mut pool = EvidencePool::default();

        // 1. 已安装应用（/Applications + ~/Applications）
        pool.installed_apps = collect_installed_apps();

        // 2. 包管理器（brew + npm -g + pip + cargo）
        pool.packages = collect_packages();

        // 3. 编辑器扩展（VS Code + JetBrains）
        pool.extensions = collect_extensions();

        // 4. 运行中的进程（简化版）
        pool.processes = collect_processes();

        pool
    }
}

/// 收集已安装应用（/Applications + ~/Applications）
fn collect_installed_apps() -> Vec<InstalledApp> {
    let mut apps = Vec::new();

    let home = std::env::var("HOME").unwrap_or_default();
    let home_apps = format!("{}/Applications", home);
    let app_dirs = [Path::new("/Applications"), Path::new(home_apps.as_str())];

    for base in app_dirs {
        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "app") {
                    let name = path
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown");
                    apps.push(InstalledApp {
                        name: name.to_string(),
                        identifier: format!("com.{}", name.to_lowercase().replace(" ", ".")),
                        version: None,
                        path: Some(path.clone()),
                    });
                }
            }
        }
    }

    // TODO: 调用 `mas list` 获取 Mac App Store 应用
    // TODO: 解析 .app/Contents/Info.plist 获取 CFBundleIdentifier

    apps
}

/// 收集包管理器安装的包（brew + npm -g）
fn collect_packages() -> Vec<Package> {
    let mut packages = Vec::new();

    // Homebrew formula
    if let Ok(output) = std::process::Command::new("brew")
        .args(&["list", "--formula"])
        .output()
    {
        if let Ok(stdout) = String::from_utf8(output.stdout) {
            for line in stdout.lines() {
                if !line.is_empty() {
                    packages.push(Package {
                        manager: "brew".to_string(),
                        name: line.to_string(),
                        version: None,
                    });
                }
            }
        }
    }

    // Homebrew cask
    if let Ok(output) = std::process::Command::new("brew")
        .args(&["list", "--cask"])
        .output()
    {
        if let Ok(stdout) = String::from_utf8(output.stdout) {
            for line in stdout.lines() {
                if !line.is_empty() {
                    packages.push(Package {
                        manager: "brew-cask".to_string(),
                        name: line.to_string(),
                        version: None,
                    });
                }
            }
        }
    }

    // npm -g
    if let Ok(output) = std::process::Command::new("npm")
        .args(&["list", "-g", "--depth=0"])
        .output()
    {
        if let Ok(stdout) = String::from_utf8(output.stdout) {
            for line in stdout.lines() {
                if line.contains("/node_modules/") {
                    if let Some(name) = line.split(" node_modules@").next() {
                        let name = name.trim().trim_start_matches('+').trim();
                        if !name.is_empty() && !name.starts_with("│") {
                            packages.push(Package {
                                manager: "npm".to_string(),
                                name: name.to_string(),
                                version: None,
                            });
                        }
                    }
                }
            }
        }
    }

    // TODO: pip、cargo、gem

    packages
}

/// 收集编辑器扩展（VS Code + JetBrains）
fn collect_extensions() -> Vec<Extension> {
    let mut extensions = Vec::new();

    // VS Code 扩展（~/.vscode/extensions）
    let home = std::env::var("HOME").unwrap_or_default();
    let vscode_ext_path_str = format!("{}/.vscode/extensions", home);
    let vscode_ext_path = Path::new(vscode_ext_path_str.as_str());

    if let Ok(entries) = std::fs::read_dir(vscode_ext_path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                // 格式: publisher.extension-name-version
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let parts: Vec<&str> = name.split('-').collect();
                if parts.len() >= 2 {
                    let ext_name = parts[1..parts.len().saturating_sub(1)].join("-");
                    extensions.push(Extension {
                        editor: "vscode".to_string(),
                        id: name.to_string(),
                        name: Some(ext_name),
                    });
                }
            }
        }
    }

    // TODO: JetBrains 插件
    // TODO: Cursor/VSCodium（同 VS Code 路径）

    extensions
}

/// 收集运行中的进程（简化版，TODO: 用 sysinfo crate）
fn collect_processes() -> Vec<ProcessInfo> {
    // 简化实现，后续用 sysinfo
    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mac_collector() {
        let collector = MacCollector;
        let pool = collector.collect();

        println!("Installed apps: {}", pool.installed_apps.len());
        println!("Packages: {}", pool.packages.len());
        println!("Extensions: {}", pool.extensions.len());

        // 至少应该能收集到一些东西
        assert!(pool.installed_apps.len() > 0 || pool.packages.len() > 0);
    }
}
