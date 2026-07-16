# DirDetective（目录侦探） — 看懂每个目录，再决定要不要清理

[English](./README.md) | **简体中文**

> 总感觉磁盘越用越少，清理工具却检测不出来？那是藏在角落的「僵尸目录」在偷偷占地方——可又不敢随便删，怕删错。
>
> 磁盘清理工具只告诉你"哪里占地方"。DirDetective 告诉你每个目录**这是谁的、存了什么、删了会怎样**——看懂了再决定要不要清理。

面向开发者与高级用户：家目录/缓存/应用支持里堆了一堆看不懂的隐藏目录（`.augmentcode`、`.V2rayU`、`.dat.nosync…`），不知道哪些还在用、哪些是卸载残留。DirDetective 帮你判断每个目录的**归属、用途、可删性**，再由你决定清理，而不是一个个拿去问 AI。

当前为 **macOS 桌面应用**（Tauri，Rust + WebView），Windows/Linux 规划中。

<p align="center"><img src="docs/screenshots/目录详情.png" width="760" alt="DirDetective 目录判定详情"></p>

---

## ✨ 功能

- **三层判定漏斗**：内置规则库（免费、秒出）→ 未知项交 AI 分析 → 结果本地缓存，越用越快
- **逐目录并发分析**：批量分析带进度、剩余时间估算，可随时停止，单个失败不影响其余
- **AI 详解**：详情页可即席问 AI "这个路径是什么"，直接看模型原文
- **安全清理**：移到废纸篓（可恢复）；内置"清理防护"拦住系统/根/家目录本身
- **保护与已识别**：手动保护关键目录；确认准确的分析可锁定复用
- **规则库可更新**：官方规则以"完整路径 → 判定"的 JSON 维护，可在设置内联网检查更新，不必等 App 发版
- **多语言**：中文 / English，跟随系统，设置内可切换
- **隐私优先**：只分析目录名与文件名，**从不读取文件内容**

---

## 📸 界面预览

<table>
  <tr>
    <td width="50%" align="center" valign="top"><img src="docs/screenshots/分析中.png" width="100%"><br><b>批量并发分析</b><br>进度 / 剩余时间估算 / 可随时停止</td>
    <td width="50%" align="center" valign="top"><img src="docs/screenshots/问问AI.png" width="100%"><br><b>即席追问 AI</b><br>详情页直接问「这个路径是什么」</td>
  </tr>
</table>

**设置面板**

<table>
  <tr>
    <td width="25%" align="center" valign="top"><img src="docs/screenshots/设置-模型.png" width="100%"><br><b>模型配置</b><br>厂家 / Base URL / Key</td>
    <td width="25%" align="center" valign="top"><img src="docs/screenshots/设置-官方规则库.png" width="100%"><br><b>官方规则库</b><br>联网检查更新</td>
    <td width="25%" align="center" valign="top"><img src="docs/screenshots/设置-清理防护.png" width="100%"><br><b>清理防护</b><br>拦截系统/根/家目录</td>
    <td width="25%" align="center" valign="top"><img src="docs/screenshots/设置-保护目录.png" width="100%"><br><b>保护目录</b><br>手动锁定关键目录</td>
  </tr>
  <tr>
    <td width="25%" align="center" valign="top"><img src="docs/screenshots/设置-已识别目录.png" width="100%"><br><b>已识别目录</b><br>锁定复用准确结果</td>
    <td width="25%" align="center" valign="top"><img src="docs/screenshots/设置-关于.png" width="100%"><br><b>关于</b><br>版本 / 检查更新 / GitHub</td>
    <td width="25%" align="center" valign="top"><img src="docs/screenshots/设置-基本.png" width="100%"><br><b>基本设置</b><br>语言 / 界面偏好</td>
    <td width="25%" align="center" valign="top"></td>
  </tr>
</table>

---

## 🚀 直接使用（一般用户）

1. 到 [Releases](https://github.com/gebilaoman/DirDetective/releases) 下载最新 macOS 安装包（`.dmg` / `.app`），拖到「应用程序」。
2. **首次打开若提示"已损坏"**：这是未签名应用被 macOS Gatekeeper 拦截（并非真损坏；**原因是目前还没有 Apple 个人开发者证书，App 未做签名与公证，所以需要这步手动放行**）。终端执行一次即可：
   ```bash
   xattr -dr com.apple.quarantine /Applications/DirDetective.app
   ```
   之后正常双击打开。
3. **配置 AI（可选，用于分析未知目录）**：打开 App → 设置 → 模型配置，选择厂家（智谱 GLM / OpenAI / DeepSeek / OpenRouter / 自定义 OpenAI 兼容服务），填入 Base URL、API Key、模型，保存。
   - 智谱 API Key：https://open.bigmodel.cn/ 注册后创建。
   - Key 只保存在本机配置文件（`~/Library/Application Support/DirDetective/config.json`）。
4. **开始用**：侧边栏选一个扫描位置（缓存 / 应用支持 / 家目录 / 自定义），或点「AI分析目录」，逐项查看归属与可删性，需要时「清理」移到废纸篓。

> 没有 AI Key 也能用：内置规则库会直接判定常见目录，未知项才需要 AI。

---

## 🛠 从源码构建 / 开发

### 依赖
- [Rust](https://rustup.rs/)（stable）
- [Node.js](https://nodejs.org/) 20+
- [pnpm](https://pnpm.io/) 9+
- macOS：Xcode Command Line Tools（`xcode-select --install`）

### 开发模式（热更新）
```bash
git clone https://github.com/gebilaoman/DirDetective.git
cd DirDetective/code/DirDetective/gui
pnpm install
pnpm tauri dev
```
前端（`gui/main.js`、`index.html`、`styles.css`）改动热更新；Rust（`crates/*`、`gui/src-tauri`）改动自动重编。

### 打包
```bash
cd gui
pnpm tauri build            # 产物在 gui/src-tauri/target/release/bundle/
pnpm tauri:build:signed     # 带更新签名（需本地 gui/.env.secret 私钥）
```

### 附带的实验性 CLI
```bash
cargo run -p dirdetective -- scan
```

### 项目结构
```
code/DirDetective/
├── crates/
│   ├── core/         # 平台无关：Scanner / RuleEngine / AIProvider / Models
│   ├── platform/     # 证据采集（已装应用/包/扩展）
│   └── cli/          # 实验性命令行入口
├── gui/              # Tauri 桌面应用（前端 + src-tauri 后端）
└── rules/            # 官方规则库（按平台分文件，见下）
```

---

## 🔒 隐私与安全

- **只上传元数据，绝不上传文件内容**：目录名、文件名采样、大小、修改时间，以及本机已装应用/包/扩展清单。
- **路径脱敏**：家目录前缀替换为 `~` 再发给 AI。
- **清理防护**（设置 → 清理防护）：系统目录（`/System`、`/Library`、`/usr`…）、根目录、家目录本身及其上级绝不允许删除；只允许清理这些之外的具体项。
- **可恢复**：清理只移到废纸篓，永远可撤销。

---

## 📚 规则库

官方规则库以"**完整路径 → 判定**"的 JSON 维护（与本地缓存同构），按平台分文件：`rules/macos.json`（Windows/Linux 规划中）。

```jsonc
{
  "version": 1,
  "rules": {
    "~/.your-tool": {
      "owner": "Your Tool",
      "purpose": "Your Tool 的本地缓存",
      "deletable": "safe",           // safe / caution / never / unknown
      "delete_effect": "删除后会重新生成"
    }
  }
}
```

**如何贡献规则**：编辑 `rules/macos.json` 增加条目 → 递增顶部 `version` → `cargo test -p dirdetective-core` 通过 → 提 PR。
合并发布后，用户在**设置 → 规则库 → 检查更新**即可联网拉到，无需更新 App。

---

## 🗺 路线图

| 版本 | 内容 |
|---|---|
| v0.1 | ✅ 桌面应用（扫描 / 规则 + AI 分析 / 缓存 / 清理 / 设置） |
| v0.1.x | ✅ 规则库路径化 JSON + 联网更新、i18n 框架、更新器 |
| **v0.2** | 🚧 i18n 全量（动态文案 + 后端提示）、规则库扩充（当前进行中） |
| 后续 | Windows/Linux 适配、社区规则字典、Apple 签名公证 |

---

## 🤝 贡献

欢迎贡献规则（见上）、报告 Bug、提建议与 PR。

## License

[MIT](LICENSE)
