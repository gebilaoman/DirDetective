// dirdetective-platform: 平台相关的证据收集
// - EvidenceCollector trait
// - MacCollector（v0.1）
// - WinCollector（v0.4）

pub mod collector;

pub use collector::{EvidenceCollector, MacCollector};
