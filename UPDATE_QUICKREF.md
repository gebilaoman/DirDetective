# DirDetective 自动更新系统 - 快速参考

## ✅ 已完成配置

### 1. 密钥生成
- ✅ 生成签名密钥对（密码：dirdetective）
- ✅ 公钥配置在 `tauri.conf.json`
- ✅ 私钥保存在 `.env.secret`（已添加到 .gitignore）

### 2. 代码配置
- ✅ 添加 `tauri-plugin-updater` 依赖
- ✅ 在 `lib.rs` 中注册 updater 插件
- ✅ 前端实现检查更新、下载、安装功能
- ✅ "关于"面板显示版本信息和更新状态

### 3. 构建配置
- ✅ `createUpdaterArtifacts: true` 启用更新文件生成
- ✅ 配置 GitHub Releases 作为更新源
- ✅ 添加签名构建脚本 `tauri:build:signed`

## 🚀 发布流程

### 快速发布（推荐）
```bash
cd gui
# 1. 更新版本号（编辑以下文件）
#    - src-tauri/tauri.conf.json
#    - src-tauri/Cargo.toml

# 2. 构建签名版本
pnpm tauri:build:signed

# 3. 发布到 GitHub
git tag v0.1.0
git push origin v0.1.0

# 4. 在 GitHub Releases 上传构建产物
#    位于: src-tauri/target/release/bundle/
```

### 手动发布
```bash
# 加载环境变量
export TAURI_SIGNING_PRIVATE_KEY="..."
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="dirdetective"

# 构建
pnpm tauri build
```

## 📋 更新文件清单

构建完成后，在 `src-tauri/target/release/bundle/` 下找到：

**macOS:**
- `dmg/DirDetective_0.1.0_aarch64.dmg`
- `dmg/DirDetective_0.1.0_aarch64.dmg.sig`

**Windows:**
- `msi/DirDetective_0.1.0_x64_en-US.msi`
- `msi/DirDetective_0.1.0_x64_en-US.msi.sig`

**Linux:**
- `appimage/DirDetective_0.1.0_amd64.AppImage`
- `appimage/DirDetective_0.1.0_amd64.AppImage.sig`

**更新清单:**
- `update.json` 或 `latest.json`

## 🔧 验证更新

### 测试步骤
1. 安装当前版本
2. 构建并发布新版本（+0.0.1）
3. 在应用中点击"检查更新"
4. 验证下载进度显示
5. 验证安装和重启

### 调试命令
```bash
# 检查配置
cat gui/src-tauri/tauri.conf.json | grep -A5 updater

# 测试构建
cd gui && pnpm tauri build --debug

# 检查构建产物
ls -la gui/src-tauri/target/release/bundle/
```

## ⚠️ 安全提醒

1. **私钥保护**：`.env.secret` 永远不要提交到 Git
2. **密钥备份**：将私钥和密码备份到安全位置
3. **定期轮换**：建议每 6-12 个月重新生成密钥
4. **泄露处理**：如私钥泄露，立即重新生成并更新配置

## 📚 相关文档

- 详细指南：`UPDATE_GUIDE.md`
- Tauri 更新文档：https://v2.tauri.app/plugin/updater/
- GitHub Releases：https://github.com/gebilaoman/DirDetective/releases
