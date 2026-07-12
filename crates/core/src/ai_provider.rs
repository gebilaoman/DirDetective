use crate::models::{
    Deletable, DirectoryMeta, EvidencePool, PROMPT_VERSION, SCHEMA_VERSION, Verdict, VerdictSource,
};
use crate::path_utils::{normalize_path_for_storage, safe_sample_names};
use async_trait::async_trait;
use chrono::Utc;
use std::path::PathBuf;

/// AI 提供者 trait
#[async_trait]
pub trait AIProvider: Send + Sync {
    /// 批量分析目录，返回判定结果
    async fn analyze(
        &self,
        dirs: Vec<DirectoryMeta>,
        evidence: &EvidencePool,
    ) -> Vec<(PathBuf, Verdict)>;
}

/// 智谱 AI 提供者（v0.2）
pub struct ZhipuAIProvider {
    api_key: String,
    model: String,
    base_url: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AIAnalysisDebug {
    pub prompt: String,
    pub raw_response: String,
    pub verdict: Verdict,
}

impl ZhipuAIProvider {
    /// 创建新的智谱 AI 提供者
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "glm-5.2".to_string(), // 使用 GLM-5.2 模型
            base_url: "https://open.bigmodel.cn/api/paas/v4".to_string(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }

    /// 从 keyring 加载 API key
    pub fn from_keyring() -> Result<Self, String> {
        let service = "dirdetective";
        let username = "zhipu-api-key";

        let entry = keyring::Entry::new(service, username)
            .map_err(|e| format!("无法创建密钥链条目: {}", e))?;

        entry
            .get_password()
            .map(|api_key| Self::new(api_key))
            .map_err(|e| format!("无法从密钥链加载 API key: {}", e))
    }

    /// 保存 API key 到 keyring
    pub fn save_to_keyring(api_key: &str) -> Result<(), String> {
        let service = "dirdetective";
        let username = "zhipu-api-key";

        let entry = keyring::Entry::new(service, username)
            .map_err(|e| format!("无法创建密钥链条目: {}", e))?;

        entry
            .set_password(api_key)
            .map_err(|e| format!("无法保存 API key 到密钥链: {}", e))
    }

    pub async fn analyze_with_debug(
        &self,
        dir: DirectoryMeta,
        evidence: &EvidencePool,
    ) -> Result<AIAnalysisDebug, String> {
        let prompt = self.build_prompt(std::slice::from_ref(&dir), evidence);
        let raw_response = self.call_api_raw(&prompt).await?;
        let mut verdict = self
            .parse_response(&raw_response)?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| "AI 返回中没有可用判定".to_string())?;
        verdict.key = normalize_path_for_storage(&dir.path);
        verdict.dir_name = dir.name.clone();
        verdict.model_id = Some(self.model.clone());
        verdict.analyzed_at = Utc::now();

        Ok(AIAnalysisDebug {
            prompt,
            raw_response,
            verdict,
        })
    }

    /// 即席自由提问：发送任意 prompt，返回模型原始文本（不解析、不缓存）。
    pub async fn ask_raw(&self, prompt: &str) -> Result<String, String> {
        self.call_api_raw(prompt).await
    }

    /// 构建 analysis prompt
    pub fn build_prompt(&self, dirs: &[DirectoryMeta], _evidence: &EvidencePool) -> String {
        let mut prompt = String::from(
            "你是磁盘目录分析助手。请先根据目录名与路径推断它属于哪个具体软件或厂商，再判断它的用途与可删性。返回 JSON。\n\n",
        );

        prompt.push_str("识别要点：\n");
        prompt.push_str(
            "- 目录名往往能直接指明归属，例如 WPKCaches→WPS Office、com.apple.Safari→Safari、.augmentcode→Augment Code、Code Cache→VS Code/Electron 应用；中国区常见软件（WPS、QQ、微信、网易、字节、腾讯等）也要尽量识别。\n",
        );
        prompt.push_str("- 可以根据目录名/路径推断归属软件，但不要罗列具体文件名，也不要猜测文件里的内容。\n\n");

        prompt.push_str("字段要求：\n");
        prompt.push_str("- owner: 给出**具体**的软件/厂商名称（如 “WPS Office”）；实在无法判断才写 null，禁止写“某个软件”“未知应用”这类含糊说法。\n");
        prompt.push_str("- purpose: 具体说明这个目录是什么、存了哪类数据（缓存/配置/备份/日志/模型等），别用泛泛的比喻，一两句讲清即可。\n");
        prompt.push_str("- delete_effect: 具体说明删除后的影响——能否被应用重新生成、会丢失什么（如自动恢复记录、登录状态、已下载内容）。\n");
        prompt.push_str("- deletable: safe(可删)/caution(谨慎)/never(保留)/unknown(不确定)\n");
        prompt.push_str("- confidence: 仅在确有把握时返回 0 到 1 的真实置信度，否则返回 null\n");
        prompt.push_str("- reason: 必须说明判断依据（基于目录名/路径的哪些线索），不得为空\n\n");

        prompt.push_str("## 待分析目录\n");
        for dir in dirs {
            let display_path = std::env::var_os("HOME").map(PathBuf::from).map_or_else(
                || dir.path.display().to_string(),
                |home| redact_home_path(&dir.path, &home),
            );
            prompt.push_str(&format!(
                "## {}\n- 类型: {}\n- 路径: {}\n- 大小: {}\n- 修改时间: {}\n",
                dir.name,
                if dir.is_directory { "目录" } else { "文件" },
                display_path,
                format_size(dir.size),
                dir.last_modified.format("%Y-%m-%d %H:%M")
            ));

            let safe_samples = safe_sample_names(&dir.top_level_samples);
            if !safe_samples.is_empty() {
                prompt.push_str("- 顶层文件: ");
                prompt.push_str(&safe_samples.join(", "));
                prompt.push('\n');
            }
            prompt.push('\n');
        }

        prompt.push_str("## 返回格式（纯 JSON）\n");
        prompt.push_str(
            r#"[{"name":"目录名","owner":"所属软件","purpose":"自然语言解释","delete_effect":"删除后果","deletable":"safe","confidence":null,"reason":"判断依据"}]"#,
        );

        prompt
    }
}

#[async_trait]
impl AIProvider for ZhipuAIProvider {
    async fn analyze(
        &self,
        dirs: Vec<DirectoryMeta>,
        evidence: &EvidencePool,
    ) -> Vec<(PathBuf, Verdict)> {
        if dirs.is_empty() {
            return vec![];
        }

        let prompt = self.build_prompt(&dirs, evidence);

        // 调用智谱 API
        match self.call_api(&prompt).await {
            Ok(verdicts) => {
                eprintln!("✅ AI 返回 {} 个判定", verdicts.len());
                let mut results = Vec::new();
                for (dir, verdict_opt) in dirs.iter().zip(verdicts.iter()) {
                    if let Some(verdict) = verdict_opt {
                        let mut verdict = verdict.clone();
                        verdict.key = normalize_path_for_storage(&dir.path);
                        verdict.dir_name = dir.name.clone();
                        verdict.model_id = Some(self.model.clone());
                        verdict.analyzed_at = Utc::now();
                        eprintln!(
                            "  - {}: {} ({})",
                            dir.name,
                            verdict.owner.as_ref().unwrap_or(&"?".to_string()),
                            verdict.deletable as i32
                        );
                        results.push((dir.path.clone(), verdict));
                    }
                }
                results
            }
            Err(e) => {
                eprintln!("❌ AI 分析失败: {}", e);
                // 返回空，表示失败
                vec![]
            }
        }
    }
}

impl ZhipuAIProvider {
    async fn call_api(&self, prompt: &str) -> Result<Vec<Option<Verdict>>, String> {
        let content = self.call_api_raw(prompt).await?;
        self.parse_response(&content)
    }

    async fn call_api_raw(&self, prompt: &str) -> Result<String, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

        let request_body = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": 0.3,
            "top_p": 0.7,
        });

        let response = client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("API 请求失败: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "无法读取错误响应".to_string());
            return Err(format!("API 返回错误 {}: {}", status, error_text));
        }

        let response_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("解析响应失败: {}", e))?;

        // 提取 AI 返回的内容
        let content = response_json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or("响应格式错误: 缺少 content")?;

        eprintln!("📝 AI 返回内容长度: {} 字符", content.len());

        Ok(content.to_string())
    }

    fn parse_response(&self, content: &str) -> Result<Vec<Option<Verdict>>, String> {
        // 去掉可能的 markdown 代码块标记（```json ... ```）
        let content = content.trim();
        let content = if content.starts_with("```json") {
            content["```json".len()..].trim()
        } else if content.starts_with("```") {
            content["```".len()..].trim()
        } else {
            content
        };
        let content = if content.ends_with("```") {
            content[..content.len() - "```".len()].trim()
        } else {
            content
        };

        eprintln!("📝 去掉 markdown 后内容长度: {} 字符", content.len());

        // 解析 JSON 数组
        let ai_verdicts: Vec<AIVerdict> = serde_json::from_str(content)
            .map_err(|e| format!("解析 AI 返回的 JSON 失败: {} (处理后内容: {})", e, content))?;

        Ok(ai_verdicts.into_iter().map(|v| v.into()).collect())
    }
}

/// AI 返回的判定格式
#[derive(Debug, serde::Deserialize)]
struct AIVerdict {
    #[allow(dead_code)]
    name: String,
    owner: String,
    purpose: String,
    delete_effect: String,
    deletable: String,
    #[serde(default)]
    confidence: Option<f32>,
    #[serde(default)]
    reason: String,
}

impl From<AIVerdict> for Option<Verdict> {
    fn from(v: AIVerdict) -> Self {
        let deletable = match v.deletable.as_str() {
            "safe" => Deletable::Safe,
            "caution" => Deletable::Caution,
            "never" => Deletable::Never,
            _ => Deletable::Unknown,
        };

        Some(Verdict {
            key: String::new(),
            dir_name: v.name,
            owner: Some(v.owner),
            purpose: v.purpose,
            delete_effect: v.delete_effect,
            deletable,
            confidence: v.confidence,
            source: VerdictSource::AI,
            reason: if v.reason.trim().is_empty() {
                "模型未提供判断依据".to_string()
            } else {
                v.reason
            },
            evidence: Vec::new(),
            is_residue: None,
            model_id: None,
            schema_version: SCHEMA_VERSION,
            prompt_version: PROMPT_VERSION,
            analyzed_at: Utc::now(),
            locked: false,
        })
    }
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn redact_home_path(path: &std::path::Path, home: &std::path::Path) -> String {
    match path.strip_prefix(home) {
        Ok(relative) if relative.as_os_str().is_empty() => "~".to_string(),
        Ok(relative) => format!("~/{}", relative.display()),
        Err(_) => path.display().to_string(),
    }
}

/// v0.1 用的空实现（保留用于测试）
pub struct StubAIProvider;

#[async_trait]
impl AIProvider for StubAIProvider {
    async fn analyze(
        &self,
        dirs: Vec<DirectoryMeta>,
        _evidence: &EvidencePool,
    ) -> Vec<(PathBuf, Verdict)> {
        tracing::warn!(
            "StubAIProvider called with {} dirs (not implemented)",
            dirs.len()
        );
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::redact_home_path;
    use std::path::Path;

    #[test]
    fn prompt_path_keeps_context_without_exposing_home() {
        let path = Path::new("/Users/example/Library/Caches/.lingma");
        let home = Path::new("/Users/example");

        let display = redact_home_path(path, home);

        assert_eq!(display, "~/Library/Caches/.lingma");
        assert!(!display.contains("example"));
    }
}
