// dirdetective CLI: v0.2 + 智谱 AI
use clap::Parser;
use comfy_table::{Attribute, Cell, Color, Table};
use dirdetective_core::{AIProvider, DirectoryMeta, ZhipuAIProvider, rule_engine, scanner};
use dirdetective_platform::{EvidenceCollector, MacCollector};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "dirdetective")]
#[command(about = "磁盘归属分析与清理工具 v0.2", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Parser, Debug)]
enum Commands {
    /// 扫描并分析目录
    Scan {
        /// 扫描路径（默认: ~ + ~/Library/Caches + ~/Library/Application Support）
        #[arg(short, long)]
        paths: Option<Vec<String>>,
        /// 启用 AI 分析（智谱 GLM）
        #[arg(long)]
        ai: bool,
        /// AI 分析的批量大小（默认 20）
        #[arg(long, default_value = "20")]
        ai_batch: usize,
    },
    /// 配置 API Key
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Parser, Debug)]
enum ConfigAction {
    /// 设置智谱 API Key
    SetZhipuKey {
        /// API Key（不提供则交互式输入）
        api_key: Option<String>,
    },
    /// 显示当前配置
    Show,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan {
            paths,
            ai,
            ai_batch,
        } => {
            let scan_paths = if let Some(p) = paths {
                p.into_iter().map(|s| PathBuf::from(s)).collect()
            } else {
                // 默认扫描路径
                let home = std::env::var("HOME").unwrap_or_default();
                vec![
                    PathBuf::from(&home),
                    PathBuf::from(format!("{}/Library/Caches", home)),
                    PathBuf::from(format!("{}/Library/Application Support", home)),
                ]
            };

            run_scan(&scan_paths, ai, ai_batch).await;
        }
        Commands::Config { action } => {
            run_config(action);
        }
    }
}

fn run_config(action: ConfigAction) {
    match action {
        ConfigAction::SetZhipuKey { api_key } => {
            let key = if let Some(k) = api_key {
                k
            } else {
                println!("请输入智谱 API Key:");
                let mut input = String::new();
                std::io::stdin()
                    .read_line(&mut input)
                    .expect("读取输入失败");
                input.trim().to_string()
            };

            if key.is_empty() {
                eprintln!("API Key 不能为空");
                return;
            }

            match ZhipuAIProvider::save_to_keyring(&key) {
                Ok(_) => {
                    println!("✅ API Key 已保存到系统密钥链");
                    println!("使用 `dirdetective scan --ai` 启用 AI 分析");
                }
                Err(e) => {
                    eprintln!("❌ 保存失败: {}", e);
                }
            }
        }
        ConfigAction::Show => {
            println!("当前配置:\n");

            match ZhipuAIProvider::from_keyring() {
                Ok(_) => {
                    println!("✅ 智谱 API Key: 已配置");
                }
                Err(_) => {
                    println!("⚠️  智谱 API Key: 未配置");
                    println!("  运行 `dirdetective config set-zhipu-key` 来设置");
                }
            }
        }
    }
}

async fn run_scan(paths: &[PathBuf], enable_ai: bool, ai_batch: usize) {
    // 转换为 &Path 引用
    let path_refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();

    // 1. 扫描目录
    println!("扫描中...\n");
    let (dirs, errors) = scanner::scan_paths(&path_refs);

    if !errors.is_empty() {
        eprintln!("⚠️  跳过 {} 个目录（权限拒绝或符号链接成环）", errors.len());
    }

    if dirs.is_empty() {
        println!("未找到任何隐藏目录");
        return;
    }

    println!("找到 {} 个目录\n", dirs.len());

    // 2. 收集证据
    println!("收集中...");
    let collector = MacCollector;
    let evidence = collector.collect();
    println!(
        "已装应用: {}, 包: {}, 扩展: {}\n",
        evidence.installed_apps.len(),
        evidence.packages.len(),
        evidence.extensions.len()
    );

    // 3. 加载内置种子规则库
    let engine = rule_engine::RuleEngine::seed();

    // 4. 先用本地规则判定
    let mut results: Vec<(DirectoryMeta, dirdetective_core::models::Verdict)> = dirs
        .into_iter()
        .map(|dir| {
            let verdict = engine.judge(&dir, &evidence);
            (dir, verdict)
        })
        .collect();

    // 5. AI 分析未知目录（如果启用）
    let unknown_count = results
        .iter()
        .filter(|(_, v)| v.source == dirdetective_core::models::VerdictSource::Unknown)
        .count();

    if enable_ai && unknown_count > 0 {
        println!("AI 分析中... ({} 个未知目录)\n", unknown_count);

        // 加载 AI provider
        let ai_provider = match ZhipuAIProvider::from_keyring() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("⚠️  无法加载 AI API Key: {}", e);
                eprintln!("  运行 `dirdetective config set-zhipu-key` 来配置");
                eprintln!("  继续使用本地规则...\n");
                print_table(&results, false);
                // 即使没有 AI，也要计算大小
                calculate_and_print_sizes(&mut results).await;
                return;
            }
        };

        // 批量分析未知目录
        let unknown_dirs: Vec<_> = results
            .iter()
            .filter(|(_, v)| v.source == dirdetective_core::models::VerdictSource::Unknown)
            .map(|(dir, _)| dir.clone())
            .collect();

        let mut ai_results = Vec::new();
        for chunk in unknown_dirs.chunks(ai_batch) {
            let chunk_results = ai_provider.analyze(chunk.to_vec(), &evidence).await;
            ai_results.extend(chunk_results);
        }

        // 更新 AI 分析结果
        if !ai_results.is_empty() {
            println!("AI 识别了 {} 个目录\n", ai_results.len());
            for (path, ai_verdict) in ai_results {
                if let Some(idx) = results.iter().position(|(dir, _)| dir.path == path) {
                    results[idx].1 = ai_verdict;
                }
            }
        }
    }

    // 6. 第一阶段：先输出表格（大小显示为"计算中"）
    print_table(&results, false);

    // 7. 第二阶段：异步计算大小并重新输出
    calculate_and_print_sizes(&mut results).await;
}

/// 异步计算大小并重新输出表格
async fn calculate_and_print_sizes(
    results: &mut [(DirectoryMeta, dirdetective_core::models::Verdict)],
) {
    println!("计算大小中...\n");

    // 提取目录引用用于批量计算（需要转换为 &[DirectoryMeta]）
    let dirs: Vec<DirectoryMeta> = results.iter().map(|(d, _)| d.clone()).collect();

    // 异步计算大小
    let sizes = dirdetective_core::scanner::calculate_sizes(&dirs).await;

    // 更新大小
    for (i, (dir, _)) in results.iter_mut().enumerate() {
        if i < sizes.len() {
            dir.size = sizes[i];
        }
    }

    println!("\n--- 大小计算完成 ---\n");

    // 重新输出表格（带真实大小）
    print_table(results, true);
}

fn print_table(results: &[(DirectoryMeta, dirdetective_core::models::Verdict)], show_sizes: bool) {
    let mut table = Table::new();
    table.load_preset(comfy_table::presets::UTF8_FULL_CONDENSED);
    table.set_header(vec!["目录", "说明", "归属", "状态", "建议", "来源", "大小"]);

    // 按大小降序、可删优先（仅在显示真实大小时排序）
    let mut sorted: Vec<_> = results.to_vec();
    if show_sizes {
        sorted.sort_by(|a, b| {
            let (v_a, v_b) = (&a.1, &b.1);
            let is_deletable_a =
                matches!(v_a.deletable, dirdetective_core::models::Deletable::Safe);
            let is_deletable_b =
                matches!(v_b.deletable, dirdetective_core::models::Deletable::Safe);

            if is_deletable_a && !is_deletable_b {
                return std::cmp::Ordering::Less;
            }
            if !is_deletable_a && is_deletable_b {
                return std::cmp::Ordering::Greater;
            }

            b.0.size.cmp(&a.0.size)
        });
    }

    for (dir, verdict) in sorted {
        let status = if verdict.is_residue == Some(true) {
            "⚠️ 残留"
        } else {
            "✅ 在装"
        };

        let (advice, color) = match verdict.deletable {
            dirdetective_core::models::Deletable::Safe => ("可删", Color::Green),
            dirdetective_core::models::Deletable::Caution => ("谨慎", Color::Yellow),
            dirdetective_core::models::Deletable::Never => ("保留", Color::Red),
            dirdetective_core::models::Deletable::Unknown => ("未知", Color::Grey),
        };

        // 说明：优先显示详细的purpose说明，如果为空则显示owner
        let description = if !verdict.purpose.is_empty() {
            &verdict.purpose
        } else {
            verdict.owner.as_deref().unwrap_or("未知")
        };

        // 归属：显示owner，如果为空则显示"未知"
        let owner = verdict.owner.as_deref().unwrap_or("未知");

        let size_str = if show_sizes {
            format_size(dir.size)
        } else {
            "计算中...".to_string()
        };
        let source = match verdict.source {
            dirdetective_core::models::VerdictSource::LocalRule => "规则",
            dirdetective_core::models::VerdictSource::AI => "AI",
            dirdetective_core::models::VerdictSource::Cache => "缓存",
            dirdetective_core::models::VerdictSource::Unknown => "未知",
        };

        table.add_row(vec![
            Cell::new(&dir.name).add_attribute(Attribute::Bold),
            Cell::new(description),
            Cell::new(owner),
            Cell::new(status),
            Cell::new(advice).fg(color),
            Cell::new(source),
            Cell::new(size_str),
        ]);
    }

    println!();
    println!("{table}");
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
