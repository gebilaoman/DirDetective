// RuleEngine: 官方规则库（以完整路径为键 → Verdict）。与缓存同构，命中即精确匹配路径。
use crate::models::{
    parse_deletable, Deletable, DirectoryMeta, EvidencePool, RuleEntry, RuleSet, Verdict,
    VerdictSource,
};
use crate::path_utils::normalize_path_for_storage;
use chrono::Utc;
use std::collections::HashMap;

/// 内嵌的种子规则库（编译期）。数据目录缺失/损坏时回退用它，App 更新也带一份新种子兜底。
pub const SEED_RULES_MACOS: &str = include_str!("../../../rules/macos.json");

pub struct RuleEngine {
    version: u32,
    rules: HashMap<String, RuleEntry>,
}

impl RuleEngine {
    pub fn new(rules: HashMap<String, RuleEntry>) -> Self {
        Self { version: 0, rules }
    }

    /// 从 JSON 加载规则库：`{ version, rules: { "完整路径": RuleEntry } }`。
    pub fn from_json_str(json: &str) -> Result<Self, serde_json::Error> {
        let set: RuleSet = serde_json::from_str(json)?;
        Ok(Self {
            version: set.version,
            rules: set.rules,
        })
    }

    /// 编译期内嵌的种子规则库。
    pub fn seed() -> Self {
        Self::from_json_str(SEED_RULES_MACOS).expect("内嵌种子规则库应始终是合法 JSON")
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// 只读访问规则表（路径 → 规则），用于设置页展示。
    pub fn rules(&self) -> &HashMap<String, RuleEntry> {
        &self.rules
    }

    /// 判定单个目录。
    pub fn judge(&self, dir: &DirectoryMeta, _evidence: &EvidencePool) -> Verdict {
        let key = normalize_path_for_storage(&dir.path);

        // 1. 硬保护：凭证目录绝不允许删（按名匹配，任何位置）。
        if dir.name == ".ssh" || dir.name == ".gnupg" {
            return self.credential_verdict(&key, dir);
        }

        // 2. 官方规则库：完整路径精确命中。
        if let Some(entry) = self.rules.get(&key) {
            let deletable = parse_deletable(&entry.deletable);
            let is_never = matches!(deletable, Deletable::Never);
            return Verdict {
                key: key.clone(),
                dir_name: dir.name.clone(),
                owner: entry.owner.clone(),
                purpose: entry.purpose.clone(),
                delete_effect: if entry.delete_effect.is_empty() {
                    default_delete_effect(&deletable)
                } else {
                    entry.delete_effect.clone()
                },
                deletable,
                confidence: Some(if is_never { 1.0 } else { 0.9 }),
                source: VerdictSource::LocalRule,
                reason: "命中官方规则库（完整路径）".to_string(),
                evidence: vec![format!("path:{}", key)],
                is_residue: Some(false),
                model_id: None,
                schema_version: crate::models::SCHEMA_VERSION,
                prompt_version: crate::models::PROMPT_VERSION,
                analyzed_at: Utc::now(),
                locked: is_never,
            };
        }

        // 3. 未命中 → unknown（交给 AI）。
        Verdict {
            key,
            dir_name: dir.name.clone(),
            owner: None,
            purpose: "未知目录".to_string(),
            delete_effect: String::new(),
            deletable: Deletable::Unknown,
            confidence: None,
            source: VerdictSource::Unknown,
            reason: "暂无规则匹配".to_string(),
            evidence: Vec::new(),
            is_residue: None,
            model_id: None,
            schema_version: crate::models::SCHEMA_VERSION,
            prompt_version: crate::models::PROMPT_VERSION,
            analyzed_at: Utc::now(),
            locked: false,
        }
    }

    fn credential_verdict(&self, key: &str, dir: &DirectoryMeta) -> Verdict {
        Verdict {
            key: key.to_string(),
            dir_name: dir.name.clone(),
            owner: Some("系统".to_string()),
            purpose: "密钥文件，绝对不能删".to_string(),
            delete_effect: "删除会永久丢失密钥、凭证和相关访问权限。".to_string(),
            deletable: Deletable::Never,
            confidence: Some(1.0),
            source: VerdictSource::LocalRule,
            reason: "命中内置硬保护规则".to_string(),
            evidence: vec![format!("name:{}", dir.name)],
            is_residue: Some(false),
            model_id: None,
            schema_version: crate::models::SCHEMA_VERSION,
            prompt_version: crate::models::PROMPT_VERSION,
            analyzed_at: Utc::now(),
            locked: true,
        }
    }
}

fn default_delete_effect(deletable: &Deletable) -> String {
    match deletable {
        Deletable::Safe => "删除后通常可由所属应用重新生成。".to_string(),
        Deletable::Caution => "删除可能影响本地配置或状态，请先确认用途。".to_string(),
        Deletable::Never => "该目录包含重要数据，不建议删除。".to_string(),
        Deletable::Unknown => "可删性尚未确认。".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::RuleEngine;
    use crate::models::{Deletable, DirectoryMeta, EvidencePool, RuleEntry, VerdictSource};
    use chrono::Utc;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn meta(path: &str, name: &str) -> DirectoryMeta {
        DirectoryMeta {
            path: PathBuf::from(path),
            name: name.into(),
            is_directory: true,
            size: 0,
            last_modified: Utc::now(),
            top_level_samples: Vec::new(),
            bundle_id_hint: None,
        }
    }

    #[test]
    fn protects_credentials_before_other_rules() {
        let engine = RuleEngine::new(HashMap::new());
        let verdict = engine.judge(&meta("/Users/example/.ssh", ".ssh"), &EvidencePool::default());
        assert_eq!(verdict.source, VerdictSource::LocalRule);
        assert_eq!(verdict.deletable, Deletable::Never);
        assert!(verdict.locked);
    }

    #[test]
    fn matches_rule_by_full_path_and_locks_never() {
        let mut rules = HashMap::new();
        rules.insert(
            "/Users/example/Documents".to_string(),
            RuleEntry {
                owner: Some("用户".to_string()),
                purpose: "个人目录".to_string(),
                delete_effect: String::new(),
                deletable: "never".to_string(),
            },
        );
        let engine = RuleEngine::new(rules);
        let verdict = engine.judge(
            &meta("/Users/example/Documents", "Documents"),
            &EvidencePool::default(),
        );
        assert_eq!(verdict.source, VerdictSource::LocalRule);
        assert_eq!(verdict.deletable, Deletable::Never);
        assert!(verdict.locked);
    }

    #[test]
    fn unknown_when_no_path_match() {
        let engine = RuleEngine::new(HashMap::new());
        let verdict = engine.judge(&meta("/tmp/whatever", "whatever"), &EvidencePool::default());
        assert_eq!(verdict.source, VerdictSource::Unknown);
    }

    #[test]
    fn embedded_seed_is_valid_json() {
        let engine = RuleEngine::seed();
        assert!(engine.rule_count() > 0);
        assert!(engine.version() >= 1);
    }
}
