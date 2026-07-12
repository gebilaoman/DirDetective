// dirdetective-core: 平台无关的核心逻辑
// - Scanner: 目录扫描（产出 DirectoryMeta）
// - RuleEngine: 规则引擎（内置规则 + 证据池匹配 → Verdict）
// - AIProvider trait: AI 分析接口（留空，v0.2 实现）
// - CacheDict: 缓存字典（v0.2）
// - Models: 数据模型

pub mod ai_provider;
pub mod models;
pub mod path_utils;
pub mod rule_engine;
pub mod scanner;

pub use ai_provider::{AIAnalysisDebug, AIProvider, StubAIProvider, ZhipuAIProvider};
pub use models::*;
