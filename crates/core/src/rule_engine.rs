// RuleEngine: 规则引擎（本地规则 + 证据池匹配 → Verdict）
use crate::models::{Deletable, DirectoryMeta, EvidencePool, Rule, Verdict, VerdictSource};
use crate::path_utils::normalize_path_for_storage;
use chrono::Utc;

pub struct RuleEngine {
    rules: Vec<Rule>,
}

impl RuleEngine {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    /// 从 YAML 加载规则
    pub fn from_yaml_str(yaml: &str) -> Result<Self, serde_yaml::Error> {
        let rules: Vec<Rule> = serde_yaml::from_str(yaml)?;
        Ok(Self::new(rules))
    }

    /// 判定单个目录
    pub fn judge(&self, dir: &DirectoryMeta, evidence: &EvidencePool) -> Verdict {
        let key = normalize_path_for_storage(&dir.path);
        // 1. 内置硬保护（.ssh/.gnupg → never）
        if dir.name == ".ssh" || dir.name == ".gnupg" {
            return Verdict {
                key,
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
            };
        }

        // 2. 内置规则匹配
        for rule in &self.rules {
            if self.matches_rule(dir, rule) {
                let is_installed = self.is_owner_installed(&rule.owner, evidence);
                let is_residue = if is_installed {
                    Some(false)
                } else if rule.deletable_if_uninstalled.is_some() {
                    Some(true)
                } else {
                    None
                };
                return Verdict {
                    key,
                    dir_name: dir.name.clone(),
                    owner: Some(rule.owner.clone()),
                    purpose: rule.purpose.clone(),
                    delete_effect: if rule.delete_effect.is_empty() {
                        "删除影响由内置规则的可删性等级给出，当前规则尚未提供更具体说明。"
                            .to_string()
                    } else {
                        rule.delete_effect.clone()
                    },
                    deletable: rule.parse_deletable(is_installed),
                    confidence: Some(
                        if rule.category == "private" || rule.category == "protect" {
                            1.0
                        } else {
                            0.9
                        },
                    ),
                    source: VerdictSource::LocalRule,
                    reason: format!("命中内置 {} 规则: {}", rule.category, rule.name),
                    evidence: vec![format!("name:{}", dir.name)],
                    is_residue,
                    model_id: None,
                    schema_version: crate::models::SCHEMA_VERSION,
                    prompt_version: crate::models::PROMPT_VERSION,
                    analyzed_at: Utc::now(),
                    locked: rule.category == "protect" || rule.category == "private",
                };
            }
        }

        // 3. 未命中 → 返回 unknown
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

    /// 检查目录是否匹配规则
    fn matches_rule(&self, dir: &DirectoryMeta, rule: &Rule) -> bool {
        // 精确匹配目录名
        if dir.name == rule.name {
            return true;
        }

        if rule.name == "uuid-temp" && is_uuid_like(&dir.name) {
            return true;
        }

        if rule.name == ".dat.nosync*" && dir.name.starts_with(".dat.nosync") {
            return true;
        }

        // 关键词模糊匹配
        for keyword in &rule.match_keywords {
            let dir_lower = dir.name.to_lowercase();
            if dir_lower.contains(&keyword.to_lowercase()) {
                return true;
            }
        }

        let normalized_path = dir.path.to_string_lossy().replace('\\', "/").to_lowercase();
        if rule
            .path_contains
            .iter()
            .any(|fragment| normalized_path.contains(&fragment.to_lowercase()))
        {
            return true;
        }

        // TODO: 从路径推断 bundle_id_hint 匹配
        false
    }

    /// 检查 owner 是否安装在证据池中（确定性判定）
    fn is_owner_installed(&self, owner: &str, evidence: &EvidencePool) -> bool {
        let owner_lower = owner.to_lowercase();

        // 检查已安装应用
        if evidence.installed_apps.iter().any(|app| {
            app.name.to_lowercase().contains(&owner_lower)
                || app.identifier.to_lowercase().contains(&owner_lower)
        }) {
            return true;
        }

        // 检查包管理器
        if evidence.packages.iter().any(|pkg| {
            pkg.name.to_lowercase().contains(&owner_lower)
                || pkg.manager.to_lowercase().contains(&owner_lower)
        }) {
            return true;
        }

        // 检查扩展
        if evidence.extensions.iter().any(|ext| {
            ext.id.to_lowercase().contains(&owner_lower)
                || ext
                    .name
                    .as_ref()
                    .map_or(false, |n| n.to_lowercase().contains(&owner_lower))
        }) {
            return true;
        }

        // TODO: 检查进程
        false
    }
}

fn is_uuid_like(name: &str) -> bool {
    let parts: Vec<&str> = name.split('-').collect();
    matches!(parts.as_slice(), [a, b, c, d, e]
        if a.len() == 8 && b.len() == 4 && c.len() == 4 && d.len() == 4 && e.len() == 12
            && name.chars().filter(|ch| *ch != '-').all(|ch| ch.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::RuleEngine;
    use crate::models::{Deletable, DirectoryMeta, EvidencePool, VerdictSource};
    use chrono::Utc;
    use std::path::PathBuf;

    fn meta(name: &str) -> DirectoryMeta {
        DirectoryMeta {
            path: PathBuf::from("/Users/example").join(name),
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
        let engine = RuleEngine::new(Vec::new());
        let verdict = engine.judge(&meta(".ssh"), &EvidencePool::default());
        assert_eq!(verdict.source, VerdictSource::LocalRule);
        assert_eq!(verdict.deletable, Deletable::Never);
        assert!(verdict.locked);
    }

    #[test]
    fn private_rule_is_local_and_locked() {
        let yaml = r#"- name: Documents
  owner: 用户
  category: private
  purpose: 个人目录
  delete_effect: 不分析
  deletable: never"#;
        let engine = RuleEngine::from_yaml_str(yaml).unwrap();
        let verdict = engine.judge(&meta("Documents"), &EvidencePool::default());
        assert_eq!(verdict.source, VerdictSource::LocalRule);
        assert!(verdict.locked);
    }
}
