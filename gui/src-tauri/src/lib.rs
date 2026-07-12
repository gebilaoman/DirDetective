// Prevents additional console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod update;

use dirdetective_core::models::{PROMPT_VERSION, SCHEMA_VERSION, VerdictSource};
use dirdetective_core::path_utils::normalize_path_for_storage;
use dirdetective_core::rule_engine::RuleEngine;
use dirdetective_core::{AIAnalysisDebug, AIProvider, DirectoryMeta, ZhipuAIProvider, scanner};
use dirdetective_platform::{EvidenceCollector, MacCollector};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static EVIDENCE_CACHE: OnceLock<dirdetective_core::models::EvidencePool> = OnceLock::new();

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanResult {
    pub directory: DirectoryMeta,
    pub verdict: dirdetective_core::models::Verdict,
    pub is_whitelisted: bool,
    pub cache_stale: bool,
}

#[derive(Debug, Serialize)]
pub struct ScanResponse {
    pub results: Vec<ScanResult>,
    pub error_count: usize,
    pub current_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StoredConfig {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub api_keys: HashMap<String, String>,
    #[serde(default)]
    pub custom_locations: Vec<String>,
    #[serde(default, skip_serializing)]
    pub api_key: Option<String>,
}

impl Default for StoredConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            base_url: default_base_url(),
            model: default_model(),
            api_keys: HashMap::new(),
            custom_locations: Vec::new(),
            api_key: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PublicConfig {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub has_api_key: bool,
    pub config_path: String,
    pub custom_locations: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TrashGuardRules {
    /// 家目录本身及其祖先（/、/Users 等）不可删——展示用的实际家目录路径。
    pub home: String,
    /// 系统目录：目录本身及全部子项都受保护。
    pub protected_prefixes: Vec<String>,
    /// 容器本身受保护，但其内部项目可以清理。
    pub protected_containers: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ReanalyzeResponse {
    pub verdict: dirdetective_core::models::Verdict,
    pub debug: Option<AIAnalysisDebug>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigInput {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FetchedModel {
    pub id: String,
    pub owned_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    #[serde(default)]
    pub key: String,
    pub name: String,
    pub verdict: dirdetective_core::models::Verdict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhitelistEntry {
    pub key: String,
    #[serde(alias = "protected_by_user")]
    pub protected: bool,
    pub added_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WhitelistViewEntry {
    pub key: String,
    pub verdict: Option<dirdetective_core::models::Verdict>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WhitelistFile {
    Paths(Vec<String>),
    Entries(Vec<LegacyWhitelistEntry>),
    Current(Vec<WhitelistEntry>),
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyWhitelistEntry {
    path: String,
    verdict: Option<dirdetective_core::models::Verdict>,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<FetchedModelEntry>,
}

#[derive(Debug, Deserialize)]
struct FetchedModelEntry {
    id: String,
    #[serde(default)]
    owned_by: Option<String>,
}

fn default_provider() -> String {
    "zhipu".to_string()
}

fn default_base_url() -> String {
    provider_default_base_url("zhipu").to_string()
}

fn default_model() -> String {
    "glm-5.2".to_string()
}

fn provider_default_base_url(provider: &str) -> &'static str {
    match provider {
        "openai" => "https://api.openai.com/v1",
        "deepseek" => "https://api.deepseek.com",
        "openrouter" => "https://openrouter.ai/api/v1",
        _ => "https://open.bigmodel.cn/api/paas/v4",
    }
}

fn get_config_path() -> PathBuf {
    get_data_dir().join("config.json")
}

fn get_data_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()))
        .join("DirDetective")
}

fn ensure_data_dirs() -> Result<PathBuf, String> {
    let root = get_data_dir();
    for name in ["rules", "cache", "logs"] {
        fs::create_dir_all(root.join(name))
            .map_err(|e| format!("创建 {} 目录失败: {}", name, e))?;
    }
    Ok(root)
}

fn get_legacy_config_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join(".dirdetective")
        .join("config.json")
}

fn get_whitelist_path() -> PathBuf {
    get_data_dir().join("whitelist.json")
}

fn get_knowledge_path() -> PathBuf {
    get_data_dir().join("knowledge.json")
}

fn get_cache_path() -> PathBuf {
    get_data_dir().join("cache").join("analyses.json")
}

fn read_json_or_default<T>(path: &Path) -> T
where
    T: serde::de::DeserializeOwned + Default,
{
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

fn write_private_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建数据目录失败: {}", e))?;
    }
    let content =
        serde_json::to_string_pretty(value).map_err(|e| format!("序列化 JSON 失败: {}", e))?;
    let mut options = fs::OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("打开 JSON 文件失败: {}", e))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("写入 JSON 失败: {}", e))?;
    #[cfg(unix)]
    fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o600))
        .map_err(|e| format!("设置 JSON 权限失败: {}", e))?;
    Ok(())
}

fn load_whitelist() -> Vec<WhitelistEntry> {
    match fs::read_to_string(get_whitelist_path())
        .ok()
        .and_then(|content| serde_json::from_str::<WhitelistFile>(&content).ok())
    {
        Some(WhitelistFile::Paths(paths)) => paths
            .into_iter()
            .map(|path| WhitelistEntry {
                key: normalize_path_for_storage(Path::new(&path)),
                protected: true,
                added_at: chrono::Utc::now(),
            })
            .collect(),
        Some(WhitelistFile::Entries(entries)) => {
            let mut cache = load_cache();
            let migrated: Vec<WhitelistEntry> = entries
                .into_iter()
                .map(|entry| {
                    let key = normalize_path_for_storage(Path::new(&entry.path));
                    if let Some(mut verdict) = entry.verdict {
                        verdict.key = key.clone();
                        if verdict.dir_name.is_empty() {
                            verdict.dir_name = Path::new(&entry.path)
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or("")
                                .to_string();
                        }
                        cache.entry(key.clone()).or_insert(verdict);
                    }
                    WhitelistEntry {
                        key,
                        protected: true,
                        added_at: chrono::Utc::now(),
                    }
                })
                .collect();
            let _ = save_cache(&cache);
            let _ = write_private_json(&get_whitelist_path(), &migrated);
            migrated
        }
        Some(WhitelistFile::Current(entries)) => entries,
        None => Vec::new(),
    }
}

fn load_cache() -> HashMap<String, dirdetective_core::models::Verdict> {
    read_json_or_default(&get_cache_path())
}

fn save_cache(cache: &HashMap<String, dirdetective_core::models::Verdict>) -> Result<(), String> {
    write_private_json(&get_cache_path(), cache)
}

fn built_in_rule_engine() -> RuleEngine {
    RuleEngine::from_yaml_str(include_str!("../../../crates/cli/rules/built-in.yaml"))
        .expect("内置规则必须是有效 YAML")
}

fn current_model_id(config: &StoredConfig) -> String {
    format!("{}:{}", config.provider, config.model)
}

fn cache_is_stale(
    verdict: &dirdetective_core::models::Verdict,
    dir: &DirectoryMeta,
    config: &StoredConfig,
) -> bool {
    verdict.schema_version != SCHEMA_VERSION
        || verdict.prompt_version != PROMPT_VERSION
        || verdict.model_id.as_deref() != Some(current_model_id(config).as_str())
        || dir.last_modified > verdict.analyzed_at
}

fn load_knowledge() -> HashMap<String, dirdetective_core::models::Verdict> {
    read_json_or_default(&get_knowledge_path())
}

fn load_config() -> StoredConfig {
    let config_path = get_config_path();
    let source_path = if config_path.exists() {
        config_path.clone()
    } else {
        get_legacy_config_path()
    };
    if source_path.exists() {
        if let Ok(content) = fs::read_to_string(&source_path) {
            if let Ok(mut config) = serde_json::from_str::<StoredConfig>(&content) {
                if config.base_url.trim().is_empty() {
                    config.base_url = provider_default_base_url(&config.provider).to_string();
                }
                if matches!(config.model.as_str(), "glm-4-flash" | "glm-4") {
                    config.model = default_model();
                    let _ = save_config(&config);
                }
                if let Some(api_key) = config.api_key.take().filter(|key| !key.is_empty()) {
                    config.api_keys.insert(config.provider.clone(), api_key);
                }
                if source_path != config_path || config.api_key.is_none() {
                    let _ = save_config(&config);
                }
                return config;
            }
        }
    }
    StoredConfig::default()
}

fn save_config(config: &StoredConfig) -> Result<(), String> {
    let config_path = get_config_path();
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {}", e))?;
    }
    let content =
        serde_json::to_string_pretty(config).map_err(|e| format!("序列化配置失败: {}", e))?;
    let mut options = fs::OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&config_path)
        .map_err(|e| format!("打开配置文件失败: {}", e))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("写入配置失败: {}", e))?;
    #[cfg(unix)]
    fs::set_permissions(
        &config_path,
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    )
    .map_err(|e| format!("设置配置权限失败: {}", e))?;
    Ok(())
}

fn expand_home(path: &str) -> PathBuf {
    if path == "~" {
        return PathBuf::from(std::env::var("HOME").unwrap_or_default());
    }
    if let Some(rest) = path.strip_prefix("~/") {
        return PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest);
    }
    PathBuf::from(path)
}

// 清理防护清单（前端「清理防护」设置页也读取这份，保证单一数据源）。
// 容器本身受保护，但其子项可清理。
const TRASH_PROTECTED_EXACT: &[&str] = &["/Users", "/Users/Shared", "/private", "/private/tmp", "/tmp"];
// 系统目录：目录本身及其所有子项都不允许删除（前缀式向下传染）。
const TRASH_PROTECTED_PREFIXES: &[&str] = &[
    "/System",
    "/Library",
    "/Applications",
    "/usr",
    "/bin",
    "/sbin",
    "/etc",
    "/opt",
    "/dev",
    "/Network",
    "/Volumes",
    "/cores",
    "/var",
    "/private/var",
    "/private/etc",
];

fn is_safe_trash_target(home: &Path, target: &Path) -> bool {
    if !target.is_absolute() || target == home {
        return false;
    }
    // 家目录的任一祖先（含 /、/Users）绝不允许删除。
    if home.starts_with(target) {
        return false;
    }
    let path = target.to_string_lossy();
    if TRASH_PROTECTED_EXACT.iter().any(|p| path == *p) {
        return false;
    }
    !TRASH_PROTECTED_PREFIXES
        .iter()
        .any(|p| target == Path::new(p) || target.starts_with(p))
}

fn build_provider(config: &StoredConfig) -> Result<ZhipuAIProvider, String> {
    let api_key = config
        .api_keys
        .get(&config.provider)
        .filter(|key| !key.is_empty())
        .cloned()
        .ok_or_else(|| format!("尚未配置 {} API Key", config.provider))?;
    Ok(ZhipuAIProvider::new(api_key)
        .with_model(config.model.clone())
        .with_base_url(config.base_url.clone()))
}

fn cached_evidence() -> &'static dirdetective_core::models::EvidencePool {
    EVIDENCE_CACHE.get_or_init(|| {
        let collector = MacCollector;
        collector.collect()
    })
}

fn pending_verdict() -> dirdetective_core::models::Verdict {
    dirdetective_core::models::Verdict {
        key: String::new(),
        dir_name: String::new(),
        owner: None,
        purpose: String::new(),
        delete_effect: String::new(),
        deletable: dirdetective_core::models::Deletable::Unknown,
        confidence: None,
        source: dirdetective_core::models::VerdictSource::Unknown,
        reason: "尚未分析".to_string(),
        evidence: Vec::new(),
        is_residue: None,
        model_id: None,
        schema_version: SCHEMA_VERSION,
        prompt_version: PROMPT_VERSION,
        analyzed_at: chrono::Utc::now(),
        locked: false,
    }
}

fn build_models_url_candidates(base_url: &str) -> Result<Vec<String>, String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err("Base URL 不能为空".to_string());
    }

    let last_segment = base.rsplit('/').next().unwrap_or_default();
    let is_version_segment = last_segment
        .strip_prefix('v')
        .is_some_and(|value| !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()));

    let mut candidates = if is_version_segment {
        vec![format!("{base}/models")]
    } else {
        vec![format!("{base}/v1/models"), format!("{base}/models")]
    };
    candidates.dedup();
    Ok(candidates)
}

#[tauri::command]
async fn scan_paths(paths: Vec<String>) -> Result<ScanResponse, String> {
    let path_bufs: Vec<PathBuf> = paths.into_iter().map(|path| expand_home(&path)).collect();
    let path_refs: Vec<&std::path::Path> = path_bufs.iter().map(|p| p.as_path()).collect();

    let (dirs, errors) = scanner::browse_paths(&path_refs);
    if !errors.is_empty() {
        eprintln!("扫描时遇到 {} 个错误", errors.len());
    }

    let whitelist = load_whitelist();
    let mut cache = load_cache();
    let knowledge = load_knowledge();
    let config = load_config();
    let rules = built_in_rule_engine();
    let evidence = cached_evidence();
    let mut cache_changed = false;
    let results: Vec<ScanResult> = dirs
        .into_iter()
        .map(|dir| {
            let key = normalize_path_for_storage(&dir.path);
            let rule_verdict = rules.judge(&dir, evidence);
            let (mut verdict, cache_stale) = if rule_verdict.source == VerdictSource::LocalRule {
                (rule_verdict, false)
            } else if let Some(cached) = cache.get(&key).cloned() {
                let stale = cache_is_stale(&cached, &dir, &config);
                let mut cached = cached;
                cached.source = VerdictSource::Cache;
                (cached, stale)
            } else if let Some(legacy) = knowledge.get(&dir.name).cloned() {
                let mut migrated = legacy;
                migrated.key = key.clone();
                migrated.dir_name = dir.name.clone();
                migrated.source = VerdictSource::Cache;
                migrated.schema_version = SCHEMA_VERSION;
                migrated.prompt_version = PROMPT_VERSION;
                if migrated.reason.trim().is_empty() {
                    migrated.reason = "从旧版已确认分析迁移".to_string();
                }
                cache.insert(key.clone(), migrated.clone());
                cache_changed = true;
                (migrated, true)
            } else {
                let mut pending = pending_verdict();
                pending.key = key.clone();
                pending.dir_name = dir.name.clone();
                (pending, false)
            };
            verdict.key = key.clone();
            verdict.dir_name = dir.name.clone();
            ScanResult {
                directory: dir,
                verdict,
                is_whitelisted: whitelist
                    .iter()
                    .any(|item| item.key == key && item.protected),
                cache_stale,
            }
        })
        .collect();
    if cache_changed {
        let _ = save_cache(&cache);
    }

    Ok(ScanResponse {
        results,
        error_count: errors.len(),
        current_path: path_bufs
            .first()
            .and_then(|path| fs::canonicalize(path).ok())
            .unwrap_or_else(|| path_bufs.first().cloned().unwrap_or_default())
            .display()
            .to_string(),
    })
}

#[tauri::command]
async fn pick_directory(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    // 原生 NSOpenPanel（经 tauri-plugin-dialog），比 osascript 子进程快得多。
    // 在 async 命令里调用 blocking_*：命令跑在非主线程，内部会派发到主线程弹窗，不会死锁。
    let folder = app
        .dialog()
        .file()
        .set_title("选择需要分析的目录")
        .blocking_pick_folder();
    Ok(folder
        .and_then(|path| path.into_path().ok())
        .map(|path| path.to_string_lossy().trim_end_matches('/').to_string()))
}

#[tauri::command]
fn get_whitelist_entries() -> Vec<WhitelistViewEntry> {
    let cache = load_cache();
    load_whitelist()
        .into_iter()
        .map(|entry| WhitelistViewEntry {
            verdict: cache.get(&entry.key).cloned(),
            key: entry.key,
        })
        .collect()
}

#[tauri::command]
fn get_knowledge_entries() -> Vec<KnowledgeEntry> {
    let mut entries: Vec<KnowledgeEntry> = load_cache()
        .into_iter()
        .map(|(key, verdict)| KnowledgeEntry {
            name: verdict.dir_name.clone(),
            key,
            verdict,
        })
        .collect();
    entries.sort_by(|a, b| a.key.cmp(&b.key));
    entries
}

#[tauri::command]
fn remove_knowledge_entry(name: String) -> Result<(), String> {
    let mut cache = load_cache();
    cache.remove(&name);
    save_cache(&cache)
}

#[tauri::command]
fn confirm_analyses(entries: Vec<KnowledgeEntry>) -> Result<usize, String> {
    let mut cache = load_cache();
    for entry in &entries {
        let key = if entry.key.is_empty() {
            entry.verdict.key.clone()
        } else {
            entry.key.clone()
        };
        let mut verdict = entry.verdict.clone();
        verdict.locked = true;
        cache.insert(key, verdict);
    }
    save_cache(&cache)?;
    Ok(entries.len())
}

#[tauri::command]
fn set_whitelist(
    path: String,
    enabled: bool,
    _verdict: Option<dirdetective_core::models::Verdict>,
) -> Result<bool, String> {
    let key = normalize_path_for_storage(Path::new(&path));
    let mut whitelist = load_whitelist();
    whitelist.retain(|item| item.key != key);
    if enabled {
        whitelist.push(WhitelistEntry {
            key,
            protected: true,
            added_at: chrono::Utc::now(),
        });
        whitelist.sort_by(|a, b| a.key.cmp(&b.key));
        whitelist.dedup_by(|a, b| a.key == b.key);
    }
    write_private_json(&get_whitelist_path(), &whitelist)?;
    Ok(enabled)
}

#[tauri::command]
fn start_window_drag(window: tauri::Window) -> Result<(), String> {
    window
        .start_dragging()
        .map_err(|e| format!("启动窗口拖拽失败: {}", e))
}

#[tauri::command]
fn confirm_analysis(
    name: String,
    verdict: dirdetective_core::models::Verdict,
) -> Result<(), String> {
    let mut cache = load_cache();
    let mut verdict = verdict;
    verdict.locked = true;
    cache.insert(name, verdict);
    save_cache(&cache)
}

#[tauri::command]
fn unlock_analysis(key: String) -> Result<(), String> {
    let mut cache = load_cache();
    if let Some(verdict) = cache.get_mut(&key) {
        verdict.locked = false;
    }
    save_cache(&cache)
}

#[tauri::command]
async fn calculate_sizes(dirs: Vec<DirectoryMeta>) -> Vec<u64> {
    let sizes = scanner::calculate_sizes(&dirs).await;
    sizes
}

#[tauri::command]
async fn analyze_with_ai(
    dirs: Vec<DirectoryMeta>,
) -> Result<Vec<(PathBuf, dirdetective_core::models::Verdict)>, String> {
    let config = load_config();
    let provider = build_provider(&config)?;
    let rules = built_in_rule_engine();
    let mut cache = load_cache();
    let mut results = Vec::new();
    let mut unknown = Vec::new();
    for dir in dirs {
        let verdict = rules.judge(&dir, cached_evidence());
        if verdict.source == VerdictSource::LocalRule {
            results.push((dir.path.clone(), verdict));
            continue;
        }
        let key = normalize_path_for_storage(&dir.path);
        if let Some(cached) = cache.get(&key).cloned() {
            if !cache_is_stale(&cached, &dir, &config) {
                let mut cached = cached;
                cached.source = VerdictSource::Cache;
                results.push((dir.path.clone(), cached));
                continue;
            }
        }
        unknown.push(dir);
    }
    let mut analyzed = provider.analyze(unknown, cached_evidence()).await;
    for (_, verdict) in &mut analyzed {
        verdict.model_id = Some(current_model_id(&config));
        verdict.source = VerdictSource::AI;
        cache.insert(verdict.key.clone(), verdict.clone());
    }
    if !analyzed.is_empty() {
        save_cache(&cache)?;
    }
    results.extend(analyzed);
    Ok(results)
}

#[tauri::command]
fn preview_ai_prompt(dirs: Vec<DirectoryMeta>) -> Result<String, String> {
    let config = load_config();
    let provider = build_provider(&config)?;
    let rules = built_in_rule_engine();
    let allowed: Vec<DirectoryMeta> = dirs
        .into_iter()
        .filter(|dir| rules.judge(dir, cached_evidence()).source == VerdictSource::Unknown)
        .collect();
    Ok(provider.build_prompt(&allowed, cached_evidence()))
}

#[tauri::command]
fn get_config() -> PublicConfig {
    let config = load_config();
    let _ = ensure_data_dirs();
    let has_api_key = config
        .api_keys
        .get(&config.provider)
        .is_some_and(|key| !key.is_empty());
    PublicConfig {
        provider: config.provider.clone(),
        base_url: config.base_url,
        model: config.model,
        has_api_key,
        config_path: get_config_path().display().to_string(),
        custom_locations: config.custom_locations,
    }
}

#[tauri::command]
fn save_config_command(config: ConfigInput) -> Result<PublicConfig, String> {
    if config.base_url.trim().is_empty() {
        return Err("Base URL 不能为空".to_string());
    }
    if config.model.trim().is_empty() {
        return Err("请先选择或填写模型".to_string());
    }
    let mut stored = load_config();
    if let Some(api_key) = config
        .api_key
        .as_deref()
        .filter(|key| !key.trim().is_empty())
    {
        stored
            .api_keys
            .insert(config.provider.clone(), api_key.trim().to_string());
    }
    stored.provider = config.provider.clone();
    stored.base_url = config.base_url.trim().trim_end_matches('/').to_string();
    stored.model = config.model.trim().to_string();
    stored.api_key = None;
    save_config(&stored)?;
    let has_api_key = stored
        .api_keys
        .get(&stored.provider)
        .is_some_and(|key| !key.is_empty());
    Ok(PublicConfig {
        provider: config.provider.clone(),
        base_url: config.base_url,
        model: config.model,
        has_api_key,
        config_path: get_config_path().display().to_string(),
        custom_locations: stored.custom_locations,
    })
}

#[tauri::command]
fn get_trash_guard_rules() -> TrashGuardRules {
    let home = fs::canonicalize(expand_home("~"))
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| expand_home("~").display().to_string());
    TrashGuardRules {
        home,
        protected_prefixes: TRASH_PROTECTED_PREFIXES.iter().map(|s| s.to_string()).collect(),
        protected_containers: TRASH_PROTECTED_EXACT.iter().map(|s| s.to_string()).collect(),
    }
}

#[tauri::command]
fn add_custom_location(path: String) -> Result<Vec<String>, String> {
    let mut stored = load_config();
    let path = path.trim_end_matches('/').to_string();
    if !path.is_empty() && !stored.custom_locations.contains(&path) {
        stored.custom_locations.push(path);
        save_config(&stored)?;
    }
    Ok(stored.custom_locations)
}

#[tauri::command]
fn remove_custom_location(path: String) -> Result<Vec<String>, String> {
    let mut stored = load_config();
    let target = path.trim_end_matches('/');
    stored.custom_locations.retain(|item| item != target);
    save_config(&stored)?;
    Ok(stored.custom_locations)
}

#[tauri::command]
async fn explain_path(path: String) -> Result<String, String> {
    let config = load_config();
    let provider = build_provider(&config)?;
    let prompt = format!("{}\n\n这个路径是干什么的？属于哪个软件/工具，有什么用？请用中文简明说明。", path);
    provider.ask_raw(&prompt).await
}

#[tauri::command]
fn open_in_file_manager(path: String, reveal: bool) -> Result<(), String> {
    let target = expand_home(&path);
    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("open");
        if reveal {
            cmd.arg("-R"); // 在访达中显示（选中）该项，即打开其所在目录
        }
        cmd.arg(&target)
            .spawn()
            .map_err(|e| format!("打开失败: {}", e))?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        let mut cmd = std::process::Command::new("explorer");
        if reveal {
            cmd.arg(format!("/select,{}", target.display()));
        } else {
            cmd.arg(&target);
        }
        cmd.spawn().map_err(|e| format!("打开失败: {}", e))?;
        return Ok(());
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let open_target = if reveal {
            target.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| target.clone())
        } else {
            target.clone()
        };
        std::process::Command::new("xdg-open")
            .arg(&open_target)
            .spawn()
            .map_err(|e| format!("打开失败: {}", e))?;
        Ok(())
    }
}

#[tauri::command]
fn open_data_directory() -> Result<(), String> {
    let directory = ensure_data_dirs()?;
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&directory)
            .spawn()
            .map_err(|e| format!("打开数据目录失败: {}", e))?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&directory)
            .spawn()
            .map_err(|e| format!("打开数据目录失败: {}", e))?;
        return Ok(());
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&directory)
            .spawn()
            .map_err(|e| format!("打开数据目录失败: {}", e))?;
        Ok(())
    }
}

#[tauri::command]
async fn fetch_models(
    provider: String,
    base_url: String,
    api_key: Option<String>,
) -> Result<Vec<FetchedModel>, String> {
    let stored = load_config();
    let key = api_key
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .or_else(|| stored.api_keys.get(&provider).cloned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("尚未配置 {} API Key", provider))?;
    let candidates = build_models_url_candidates(&base_url)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("创建请求客户端失败: {}", e))?;
    let mut errors = Vec::new();

    for url in candidates {
        let response = match client.get(&url).bearer_auth(&key).send().await {
            Ok(response) => response,
            Err(error) => {
                errors.push(format!("{}: {}", url, error));
                continue;
            }
        };
        let status = response.status();
        if !status.is_success() {
            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(
                    "当前 Provider 的 API Key 无效或没有模型列表权限，请重新填写并保存".to_string(),
                );
            }
            errors.push(format!("{}: HTTP {}", url, status));
            continue;
        }

        let response: ModelsResponse = response
            .json()
            .await
            .map_err(|e| format!("模型列表响应解析失败: {}", e))?;
        let mut models: Vec<FetchedModel> = response
            .data
            .into_iter()
            .map(|model| FetchedModel {
                id: model.id,
                owned_by: model.owned_by,
            })
            .collect();
        models.sort_by(|a, b| a.id.cmp(&b.id));
        models.dedup_by(|a, b| a.id == b.id);
        return Ok(models);
    }

    Err(format!("无法获取模型列表：{}", errors.join("；")))
}

#[tauri::command]
async fn reanalyze_directory(dir: DirectoryMeta) -> Result<ReanalyzeResponse, String> {
    let config = load_config();
    let provider = build_provider(&config)?;
    let rule_verdict = built_in_rule_engine().judge(&dir, cached_evidence());
    if rule_verdict.source == VerdictSource::LocalRule {
        return Ok(ReanalyzeResponse {
            verdict: rule_verdict,
            debug: None,
        });
    }

    #[cfg(debug_assertions)]
    {
        let mut debug = provider.analyze_with_debug(dir, cached_evidence()).await?;
        debug.verdict.model_id = Some(current_model_id(&config));
        let mut cache = load_cache();
        cache.insert(debug.verdict.key.clone(), debug.verdict.clone());
        save_cache(&cache)?;
        return Ok(ReanalyzeResponse {
            verdict: debug.verdict.clone(),
            debug: Some(debug),
        });
    }

    #[cfg(not(debug_assertions))]
    {
        let results = provider.analyze(vec![dir], cached_evidence()).await;
        if let Some(result) = results.first() {
            let mut verdict = result.1.clone();
            verdict.model_id = Some(current_model_id(&config));
            let mut cache = load_cache();
            cache.insert(verdict.key.clone(), verdict.clone());
            save_cache(&cache)?;
            Ok(ReanalyzeResponse {
                verdict,
                debug: None,
            })
        } else {
            Err("AI 分析失败".to_string())
        }
    }
}

#[tauri::command]
fn trash_directory(path: String, force_protected: bool) -> Result<(), String> {
    let home =
        fs::canonicalize(expand_home("~")).map_err(|e| format!("无法读取用户目录: {}", e))?;
    let target =
        fs::canonicalize(expand_home(&path)).map_err(|e| format!("目标不存在或无法访问: {}", e))?;

    // 允许删除文件与目录（trash::delete 两者皆可）；仅拦截系统/根/家目录本身。
    if !is_safe_trash_target(&home, &target) {
        return Err(format!(
            "出于安全考虑，系统目录、根目录及用户目录本身不允许移到废纸篓（目标：{}）",
            target.display()
        ));
    }

    let key = normalize_path_for_storage(&target);
    if !force_protected
        && load_whitelist()
            .iter()
            .any(|entry| entry.key == key && entry.protected)
    {
        return Err("该目标已被用户保护；取消保护后再删除，或执行额外确认。".to_string());
    }

    trash::delete(&target).map_err(|e| format!("移到废纸篓失败: {}", e))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            scan_paths,
            pick_directory,
            get_whitelist_entries,
            get_knowledge_entries,
            remove_knowledge_entry,
            confirm_analyses,
            set_whitelist,
            start_window_drag,
            confirm_analysis,
            unlock_analysis,
            calculate_sizes,
            analyze_with_ai,
            preview_ai_prompt,
            get_config,
            save_config_command,
            add_custom_location,
            remove_custom_location,
            get_trash_guard_rules,
            open_data_directory,
            open_in_file_manager,
            explain_path,
            fetch_models,
            reanalyze_directory,
            trash_directory,
            update::get_app_version,
            update::get_system_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{
        StoredConfig, WhitelistFile, build_models_url_candidates, cache_is_stale,
        is_safe_trash_target,
    };
    use chrono::{Duration, Utc};
    use dirdetective_core::models::{DirectoryMeta, Verdict};
    use std::path::Path;
    use std::path::PathBuf;

    #[test]
    fn trash_target_guard_blocks_system_and_home_ancestors() {
        let home = Path::new("/Users/example");

        // 允许：家目录内，以及家目录外的普通子目录（如 /Users/Shared 下）。
        assert!(is_safe_trash_target(
            home,
            Path::new("/Users/example/Library/Caches/example")
        ));
        assert!(is_safe_trash_target(
            home,
            Path::new("/Users/Shared/.Wondershare")
        ));
        assert!(is_safe_trash_target(
            home,
            Path::new("/Users/example-other/cache")
        ));

        // 拒绝：家目录本身、其祖先、根、以及 Users/Shared 容器本身。
        assert!(!is_safe_trash_target(home, home));
        assert!(!is_safe_trash_target(home, Path::new("/")));
        assert!(!is_safe_trash_target(home, Path::new("/Users")));
        assert!(!is_safe_trash_target(home, Path::new("/Users/Shared")));

        // 拒绝：系统目录及其子项。
        assert!(!is_safe_trash_target(home, Path::new("/Applications")));
        assert!(!is_safe_trash_target(
            home,
            Path::new("/System/Library/Caches/x")
        ));
        assert!(!is_safe_trash_target(home, Path::new("/Library/Caches/x")));
    }

    #[test]
    fn model_urls_follow_openai_compatible_conventions() {
        assert_eq!(
            build_models_url_candidates("https://open.bigmodel.cn/api/paas/v4").unwrap(),
            vec!["https://open.bigmodel.cn/api/paas/v4/models"]
        );
        assert_eq!(
            build_models_url_candidates("https://api.openai.com/v1").unwrap(),
            vec!["https://api.openai.com/v1/models"]
        );
        assert_eq!(
            build_models_url_candidates("https://api.deepseek.com").unwrap(),
            vec![
                "https://api.deepseek.com/v1/models",
                "https://api.deepseek.com/models"
            ]
        );
    }

    #[test]
    fn legacy_and_current_whitelists_are_readable() {
        assert!(serde_json::from_str::<WhitelistFile>(r#"["/Users/example/.cache"]"#).is_ok());
        assert!(
            serde_json::from_str::<WhitelistFile>(
                r#"[{"path":"/Users/example/.cache","verdict":null}]"#
            )
            .is_ok()
        );
        assert!(
            serde_json::from_str::<WhitelistFile>(
                r#"[{"key":"~/.cache","protected_by_user":true,"added_at":"2026-01-01T00:00:00Z"}]"#
            )
            .is_ok()
        );
        assert!(
            serde_json::from_str::<WhitelistFile>(
                r#"[{"key":"~/.cache","protected":true,"added_at":"2026-01-01T00:00:00Z"}]"#
            )
            .is_ok()
        );
    }

    #[test]
    fn cache_expires_when_directory_or_model_changes() {
        let now = Utc::now();
        let mut verdict: Verdict = serde_json::from_value(serde_json::json!({
            "key":"~/Library/Caches/example", "dir_name":"example", "owner":"Example",
            "purpose":"cache", "delete_effect":"rebuild", "deletable":"safe",
            "confidence":0.9, "source":"ai", "reason":"model evidence", "evidence":[],
            "is_residue":null, "model_id":"zhipu:glm-5.2", "schema_version":1,
            "prompt_version":1, "analyzed_at":now, "locked":false
        }))
        .unwrap();
        let config = StoredConfig {
            provider: "zhipu".into(),
            base_url: String::new(),
            model: "glm-5.2".into(),
            api_keys: Default::default(),
            custom_locations: Vec::new(),
            api_key: None,
        };
        let mut dir = DirectoryMeta {
            path: PathBuf::from("/Users/example/Library/Caches/example"),
            name: "example".into(),
            is_directory: true,
            size: 0,
            last_modified: now - Duration::seconds(1),
            top_level_samples: vec![],
            bundle_id_hint: None,
        };
        assert!(!cache_is_stale(&verdict, &dir, &config));
        dir.last_modified = now + Duration::seconds(1);
        assert!(cache_is_stale(&verdict, &dir, &config));
        dir.last_modified = now - Duration::seconds(1);
        verdict.model_id = Some("openai:gpt".into());
        assert!(cache_is_stale(&verdict, &dir, &config));
    }
}
