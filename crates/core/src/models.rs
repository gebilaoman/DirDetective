use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 扫描到的目录元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryMeta {
    /// 完整路径
    pub path: std::path::PathBuf,
    /// 是否为目录；GUI 浏览模式也会返回普通文件
    #[serde(default = "default_is_directory")]
    pub is_directory: bool,
    /// 目录名
    pub name: String,
    /// 总大小（字节）
    pub size: u64,
    /// 最后修改时间
    pub last_modified: DateTime<Utc>,
    /// 顶层文件名采样（最多 20 个，用于 AI 识别）
    pub top_level_samples: Vec<String>,
    /// bundle ID 提示（从路径推断，如 "com.augmentcode"）
    pub bundle_id_hint: Option<String>,
}

fn default_is_directory() -> bool {
    true
}

/// 判定结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verdict {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub dir_name: String,
    /// 归属的应用名
    pub owner: Option<String>,
    /// 目录用途（如 "VS Code 扩展数据"、"AI 编程插件账号"）
    pub purpose: String,
    #[serde(default)]
    pub delete_effect: String,
    /// 可删性判定
    pub deletable: Deletable,
    /// 置信度（0.0 - 1.0）
    #[serde(default)]
    pub confidence: Option<f32>,
    /// 判定来源（local_rule / ai / cache / unknown）
    pub source: VerdictSource,
    /// 理由（给用户看的解释）
    pub reason: String,
    #[serde(default)]
    pub evidence: Vec<String>,
    /// 是否为卸载残留（确定性判定，基于证据池）
    #[serde(default)]
    pub is_residue: Option<bool>,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default = "default_prompt_version")]
    pub prompt_version: u32,
    #[serde(default = "default_analyzed_at")]
    pub analyzed_at: DateTime<Utc>,
    #[serde(default)]
    pub locked: bool,
}

pub const SCHEMA_VERSION: u32 = 1;
pub const PROMPT_VERSION: u32 = 2;

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}
fn default_prompt_version() -> u32 {
    PROMPT_VERSION
}
fn default_analyzed_at() -> DateTime<Utc> {
    DateTime::<Utc>::from(std::time::UNIX_EPOCH)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Deletable {
    /// 安全删除（缓存/残留）
    Safe,
    /// 谨慎（可能是数据，但 app 未装）
    Caution,
    /// 保留（.ssh/.gnupg 等关键目录）
    Never,
    /// 未知
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerdictSource {
    LocalRule,
    #[serde(rename = "ai", alias = "a_i")]
    AI,
    Cache,
    Unknown,
}

/// 证据池（从 EvidenceCollector 收集）
#[derive(Debug, Clone, Default)]
pub struct EvidencePool {
    /// 已安装的应用
    pub installed_apps: Vec<InstalledApp>,
    /// 包管理器安装的包
    pub packages: Vec<Package>,
    /// 编辑器扩展
    pub extensions: Vec<Extension>,
    /// 运行中的进程
    pub processes: Vec<ProcessInfo>,
}

#[derive(Debug, Clone)]
pub struct InstalledApp {
    /// 应用名
    pub name: String,
    /// bundle ID（macOS）或注册表路径（Windows）
    pub identifier: String,
    /// 版本（可选）
    pub version: Option<String>,
    /// 安装路径（可选）
    pub path: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Package {
    /// 包管理器名称（brew / npm / pip / cargo / gem / winget / scoop / choco）
    pub manager: String,
    /// 包名
    pub name: String,
    /// 版本（可选）
    pub version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Extension {
    /// 编辑器类型（vscode / jetbrains）
    pub editor: String,
    /// 扩展 ID 或名
    pub id: String,
    /// 扩展名（可选）
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// 进程名
    pub name: String,
    /// PID
    pub pid: u32,
}

/// 扫描错误（用于报告而非静默跳过）
#[derive(Debug, Clone)]
pub enum ScanError {
    /// 权限拒绝
    PermissionDenied { path: std::path::PathBuf },
    /// 符号链接成环
    SymlinkLoop { path: std::path::PathBuf },
    /// IO 错误
    Io {
        path: std::path::PathBuf,
        error: String,
    },
}

/// 单条规则：以完整路径为键，值只描述"这是什么、能不能删"。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuleEntry {
    /// 归属应用/主体
    #[serde(default)]
    pub owner: Option<String>,
    /// 用途说明
    pub purpose: String,
    /// 删除后果（可选，为空时引擎按可删性给默认说明）
    #[serde(default)]
    pub delete_effect: String,
    /// 可删性（safe / caution / never / unknown）
    #[serde(default = "default_deletable")]
    pub deletable: String,
}

/// 规则集：`{ version, rules: { "完整路径": RuleEntry } }`（与缓存同构，以路径为键）。
#[derive(Debug, Clone, Deserialize)]
pub struct RuleSet {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub rules: std::collections::HashMap<String, RuleEntry>,
}

fn default_deletable() -> String {
    "unknown".to_string()
}

/// 把 deletable 字符串解析为 Deletable。
pub fn parse_deletable(s: &str) -> Deletable {
    match s {
        "safe" => Deletable::Safe,
        "caution" => Deletable::Caution,
        "never" => Deletable::Never,
        _ => Deletable::Unknown,
    }
}
