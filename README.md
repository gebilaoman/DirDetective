# DirDetective — AI 磁盘归属分析与清理工具

> 磁盘清理工具告诉你"哪里占地方"，DirDetective 告诉你"**这是谁的、能不能删**"。

## 产品定位

目标用户：开发者及高级用户，用户目录下堆积大量隐藏目录/缓存/残留，想快速判断"这是谁的、属于哪个应用、是否为卸载残留、能不能安全删除"。

## 核心功能

### 三层判断漏斗

```
本地规则（免费，~80% 命中）→ AI 分析（智谱 GLM）→ 本地缓存字典
```

1. **本地规则**：内置 20+ 常见目录规则（.gradle/.npm/.vscode 等），免费快速
2. **AI 分析**：未知目录批量送智谱 GLM-4-Flash 分析，注入本机证据池（已装应用/包/扩展）
3. **缓存字典**：同名目录只问一次（v0.3）

### 确定性判定

- **"在装 / 残留"**：基于本机证据池判定，不依赖 AI
- **"可删 / 谨慎 / 保留"**：结合规则与 AI 理由

## 安装

### 从源码编译

```bash
git clone <repo>
cd DirDetective/code/DirDetective
cargo build --release
```

编译后的二进制：`./target/release/dirdetective`

### Cargo 安装（开发中）

```bash
cargo install --path crates/cli
```

## 使用

### 1. 配置 AI API Key（可选）

首次使用 AI 功能需要配置智谱 API Key：

```bash
# 交互式输入（推荐，密码安全）
dirdetective config set-zhipu-key

# 或直接传入
dirdetective config set-zhipu-key your-api-key-here

# 查看当前配置
dirdetective config show
```

API Key 存储在系统密钥链（macOS Keychain / Windows Credential Manager），无需重复输入。

**获取智谱 API Key：**
- 访问 https://open.bigmodel.cn/
- 注册/登录后进入 API Key 页面
- 创建新 Key（建议选择 `glm-4-flash` 以降低成本）

### 2. 扫描磁盘

```bash
# 默认扫描（~ + ~/Library/Caches + ~/Library/Application Support）
dirdetective scan

# 指定扫描路径
dirdetective scan --paths ~/Library/Caches

# 启用 AI 分析未知目录
dirdetective scan --ai

# 自定义 AI 批量大小（默认 20）
dirdetective scan --ai --ai-batch 30
```

### 3. 输出说明

| 列 | 说明 |
|---|---|
| 目录 | 目录名 |
| 归属 | 所属应用/工具 |
| 状态 | ✅ 在装 / ⚠️ 残留 |
| 建议 | 可删（绿色）/ 谨慎（黄色）/ 保留（红色）/ 未知（灰色）|
| 来源 | 规则（本地）/ AI（智谱）/ 缓存 / 未知 |
| 大小 | 占用空间 |

**示例输出：**

```
┌────────────────────────┬──────────────┬─────────┬──────┬──────┬──────────┐
│ 目录                   ┆ 归属         ┆ 状态    ┆ 建议 ┆ 来源 ┆ 大小     │
╞════════════════════════╪══════════════╪═════════╪══════╪══════╪══════════╡
│ .m2                    ┆ Maven        ┆ ✅ 在装 ┆ 可删 ┆ 规则 ┆ 7.4 GB   │
│ .lmstudio              ┆ LM Studio    ┆ ✅ 在装 ┆ 谨慎 ┆ AI   ┆ 2.7 GB   │
│ .augmentcode           ┆ Augment Code ┆ ⚠️ 残留 ┆ 可删 ┆ 规则 ┆ 207.2 MB │
│ .ssh                   ┆ 系统         ┆ ✅ 在装 ┆ 保留 ┆ 规则 ┆ 71.7 KB  │
└────────────────────────┴──────────────┴─────────┴──────┴──────┴──────────┘
```

## 工作原理

### 证据收集（MacCollector）

- 已安装应用：`/Applications`、`~/Applications`、`brew list --cask`
- 包管理器：Homebrew（formula + cask）、npm -g
- 编辑器扩展：`~/.vscode/extensions`、JetBrains 插件
- 进程：运行中的应用（TODO）

### 规则匹配

内置规则覆盖常见开发工具：

| 目录 | 归属 | 可删性 |
|---|---|---|
| `.gradle` | Gradle | safe（缓存） |
| `.npm` | npm | caution（在用）/ safe（卸载后） |
| `.vscode` | VS Code | caution |
| `.ssh` / `.gnupg` | 系统 | never（保护） |

### AI 分析（智谱 GLM-4-Flash）

- 批量分析：每次 20 个目录（可配置）
- Prompt 注入：本机证据池（已装应用/包/扩展清单）
- 输出格式：JSON（owner、purpose、deletable、reason）
- 模型选择：`glm-4-flash`（快速便宜，适合批量分析）

## 隐私与安全

### 数据边界

**仅上传元数据，绝不上传文件内容：**
- 目录名、大小、修改时间
- 顶层文件名采样（最多 20 个）
- 本机已装应用/包/扩展清单

**路径脱敏：**
- `~` 替换为 `~`
- `%USERPROFILE%` 替换为 `%USERPROFILE%`

**删除安全：**
- 仅移至系统回收站（`trash` crate）
- 永远可撤销
- `never` 级不渲染删除按钮

### Ollama 全本地模式（v0.3）

支持 `localhost:11434` 的 Ollama，数据不出本机。

## 开发

### 项目结构

```
dirdetective/
├── crates/
│   ├── core/          # 平台无关：Scanner / RuleEngine / AIProvider / Models
│   ├── platform/      # EvidenceCollector trait + MacCollector / WinCollector
│   └── cli/           # 命令行入口
├── src/               # 前端（Tauri，v0.3）
└── 资料/              # 设计文档
```

### 运行

```bash
# 开发模式
cargo run -- scan --ai

# 构建
cargo build --release

# 测试
cargo test
```

## 路线图

| 版本 | 内容 |
|---|---|
| **v0.1** | ✅ core + platform(mac) + CLI 表格 |
| **v0.2** | ✅ AIProvider（智谱 GLM）+ 缓存字典 |
| v0.3 | Tauri GUI + 回收站删除 + 设置页（mac） |
| v0.4 | WinCollector + Windows 支持 |
| v0.5 | 社区字典拉取 + 应用内提交 |
| v0.6 | 首个公开 Release |

## 贡献

欢迎贡献规则、报告 Bug、提建议。

### 贡献规则

在 `crates/cli/rules/built-in.yaml` 中添加新规则：

```yaml
- name: ".your-tool"
  owner: "Your Tool"
  kind: "cache"
  purpose: "Your Tool 的缓存目录"
  deletable: "safe"
  match_keywords: ["your", "tool"]
  platforms: ["macos", "windows"]
```

## License

MIT

---

**磁盘空间不够？别再一个一个问 AI 了。**

`dirdetective scan --ai` 一键扫清。
